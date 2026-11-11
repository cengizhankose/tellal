// Records ONE window (by CGWindowID) with ScreenCaptureKit into a ProRes 4444 .mov with alpha.
// Nothing else on screen can appear: the capture filter is the window alone, independent of the
// desktop and of any window above or below it.
//
// The window may change height while recording (the panel sizes itself to its content); every
// frame is copied top-left aligned into a fixed `--width` x `--height` (points) transparent canvas,
// so the panel grows downward exactly as on screen.
//
// Usage: record-window --window <id> --out <file.mov> --width <pt> --height <pt>
//                      [--fps 60] [--pid <pid>] [--scale 2]
// Stops when <pid> exits, the window disappears, or on SIGINT/SIGTERM. Prints one JSON line per
// event on stdout; `start` carries the wall-clock time (unix ms) of the first frame.
//
// Build: swiftc -O tools/record-window.swift -o target/ui-dev/record-window

import AppKit
import AVFoundation
import CoreMedia
import CoreVideo
import Foundation
import ScreenCaptureKit

struct Options {
    var window: CGWindowID = 0
    var out = ""
    var width = 0.0
    var height = 0.0
    var fps = 60
    var pid: pid_t = 0
    var scale = 2.0

    static func parse() -> Options {
        var o = Options()
        var args = CommandLine.arguments.dropFirst().makeIterator()
        while let flag = args.next() {
            guard let value = args.next() else { fail("missing value for \(flag)") }
            switch flag {
            case "--window": o.window = CGWindowID(value) ?? 0
            case "--out": o.out = value
            case "--width": o.width = Double(value) ?? 0
            case "--height": o.height = Double(value) ?? 0
            case "--fps": o.fps = Int(value) ?? 60
            case "--pid": o.pid = pid_t(value) ?? 0
            case "--scale": o.scale = Double(value) ?? 2
            default: fail("unknown flag \(flag)")
            }
        }
        if o.window == 0 || o.out.isEmpty || o.width <= 0 || o.height <= 0 {
            fail("usage: record-window --window <id> --out <file.mov> --width <pt> --height <pt> [--fps 60] [--pid <pid>] [--scale 2]")
        }
        return o
    }
}

func emit(_ fields: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys]),
       let line = String(data: data, encoding: .utf8) {
        print(line)
        fflush(stdout)
    }
}

func fail(_ message: String) -> Never {
    emit(["event": "error", "error": message])
    exit(1)
}

func nowUnixMs() -> Double { Date().timeIntervalSince1970 * 1000 }

final class Recorder: NSObject, SCStreamOutput, SCStreamDelegate {
    let opts: Options
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    let adaptor: AVAssetWriterInputPixelBufferAdaptor
    let canvasW: Int
    let canvasH: Int
    let queue = DispatchQueue(label: "record-window.frames")
    var stream: SCStream?
    var started = false
    var frames = 0
    var clipped = 0
    var lastPTS = CMTime.invalid
    var finishing = false

    init(opts: Options) throws {
        self.opts = opts
        canvasW = Int((opts.width * opts.scale).rounded())
        canvasH = Int((opts.height * opts.scale).rounded())
        let url = URL(fileURLWithPath: opts.out)
        try? FileManager.default.removeItem(at: url)
        writer = try AVAssetWriter(outputURL: url, fileType: .mov)
        input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.proRes4444,
            AVVideoWidthKey: canvasW,
            AVVideoHeightKey: canvasH,
        ])
        input.expectsMediaDataInRealTime = true
        adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: canvasW,
            kCVPixelBufferHeightKey as String: canvasH,
        ])
        guard writer.canAdd(input) else { fail("cannot add ProRes 4444 input") }
        writer.add(input)
        super.init()
    }

    func start() async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
        guard let window = content.windows.first(where: { $0.windowID == opts.window }) else {
            fail("window \(opts.window) not found")
        }
        let filter = SCContentFilter(desktopIndependentWindow: window)
        let config = SCStreamConfiguration()
        config.width = canvasW
        config.height = canvasH
        config.scalesToFit = false
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.colorSpaceName = CGColorSpace.sRGB
        config.showsCursor = false
        config.capturesAudio = false
        config.ignoreShadowsSingleWindow = true
        config.shouldBeOpaque = false
        config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(opts.fps))
        config.queueDepth = 8
        let stream = SCStream(filter: filter, configuration: config, delegate: self)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: queue)
        guard writer.startWriting() else { fail("writer: \(writer.error?.localizedDescription ?? "?")") }
        try await stream.startCapture()
        self.stream = stream
        emit(["event": "capturing", "window": Int(opts.window), "canvas": [canvasW, canvasH],
              "window_frame": [window.frame.origin.x, window.frame.origin.y, window.frame.width, window.frame.height]])
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, !finishing, sample.isValid,
              let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let info = attachments.first,
              let rawStatus = info[.status] as? Int, SCFrameStatus(rawValue: rawStatus) == .complete,
              let source = CMSampleBufferGetImageBuffer(sample)
        else { return }
        let pts = CMSampleBufferGetPresentationTimeStamp(sample)
        if lastPTS.isValid && CMTimeCompare(pts, lastPTS) <= 0 { return }
        if !started {
            let hostNow = CMClockGetTime(CMClockGetHostTimeClock())
            let ageMs = CMTimeGetSeconds(CMTimeSubtract(hostNow, pts)) * 1000
            writer.startSession(atSourceTime: pts)
            started = true
            emit(["event": "start", "first_frame_unix_ms": nowUnixMs() - ageMs])
        }
        guard input.isReadyForMoreMediaData, let pool = adaptor.pixelBufferPool else { return }
        var out: CVPixelBuffer?
        guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &out) == kCVReturnSuccess, let canvas = out else { return }
        // The window's pixels inside the captured frame.
        var rect = CGRect(x: 0, y: 0, width: CVPixelBufferGetWidth(source), height: CVPixelBufferGetHeight(source))
        if let dict = info[.contentRect], let r = CGRect(dictionaryRepresentation: dict as! CFDictionary) {
            let scale = (info[.scaleFactor] as? Double) ?? 1
            rect = CGRect(x: r.origin.x * scale, y: r.origin.y * scale, width: r.width * scale, height: r.height * scale)
        }
        copyTopLeft(from: source, rect: rect, into: canvas)
        if adaptor.append(canvas, withPresentationTime: pts) {
            frames += 1
            lastPTS = pts
        }
    }

    /// Clears `canvas` to transparent and copies `rect` of `source` to its top-left corner.
    func copyTopLeft(from source: CVPixelBuffer, rect: CGRect, into canvas: CVPixelBuffer) {
        CVPixelBufferLockBaseAddress(source, .readOnly)
        CVPixelBufferLockBaseAddress(canvas, [])
        defer {
            CVPixelBufferUnlockBaseAddress(canvas, [])
            CVPixelBufferUnlockBaseAddress(source, .readOnly)
        }
        guard let src = CVPixelBufferGetBaseAddress(source), let dst = CVPixelBufferGetBaseAddress(canvas) else { return }
        let srcStride = CVPixelBufferGetBytesPerRow(source)
        let dstStride = CVPixelBufferGetBytesPerRow(canvas)
        memset(dst, 0, dstStride * canvasH)
        let x0 = max(0, Int(rect.origin.x.rounded()))
        let y0 = max(0, Int(rect.origin.y.rounded()))
        let srcW = CVPixelBufferGetWidth(source)
        let srcH = CVPixelBufferGetHeight(source)
        let w = min(Int(rect.width.rounded()), srcW - x0, canvasW)
        let h = min(Int(rect.height.rounded()), srcH - y0, canvasH)
        if Int(rect.height.rounded()) > canvasH || Int(rect.width.rounded()) > canvasW { clipped += 1 }
        guard w > 0, h > 0 else { return }
        for row in 0..<h {
            memcpy(dst + row * dstStride, src + (y0 + row) * srcStride + x0 * 4, w * 4)
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        emit(["event": "stream_stopped", "error": error.localizedDescription])
        finish()
    }

    func finish() {
        queue.async {
            if self.finishing { return }
            self.finishing = true
            Task {
                try? await self.stream?.stopCapture()
                self.queue.async {
                    self.input.markAsFinished()
                    let frames = self.frames
                    let clipped = self.clipped
                    let end = self.lastPTS
                    if end.isValid { self.writer.endSession(atSourceTime: end) }
                    self.writer.finishWriting {
                        emit(["event": "done", "frames": frames, "clipped_frames": clipped,
                              "ok": self.writer.status == .completed,
                              "error": self.writer.error?.localizedDescription ?? NSNull()])
                        exit(self.writer.status == .completed ? 0 : 1)
                    }
                }
            }
        }
    }
}

// ScreenCaptureKit needs a CoreGraphics session (CGS) like any GUI client.
_ = NSApplication.shared
let opts = Options.parse()
let recorder: Recorder
do { recorder = try Recorder(opts: opts) } catch { fail("writer: \(error.localizedDescription)") }

var signalSources: [DispatchSourceSignal] = []
var watchTimer: DispatchSourceTimer?
for sig in [SIGINT, SIGTERM] {
    signal(sig, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: sig, queue: .main)
    source.setEventHandler { recorder.finish() }
    source.resume()
    signalSources.append(source)
}

if opts.pid > 0 {
    let timer = DispatchSource.makeTimerSource(queue: .main)
    timer.schedule(deadline: .now() + 0.25, repeating: 0.25)
    timer.setEventHandler {
        if kill(opts.pid, 0) != 0 { recorder.finish() }
    }
    timer.resume()
    watchTimer = timer
}

Task {
    do { try await recorder.start() } catch { fail("capture: \(error.localizedDescription)") }
}
dispatchMain()
