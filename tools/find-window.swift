// Prints the windows of one app as JSON lines (CGWindowID, pid, title, bounds in points,
// on-screen flag), for tools/record-window.swift; windows on another Space are listed too.
// Reads the window list only; activates nothing.
//
// Usage: find-window <app name>      e.g. find-window BrowserOS
// Build: swiftc -O tools/find-window.swift -o target/hero-dev/find-window

import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write("usage: find-window <app name>\n".data(using: .utf8)!)
    exit(2)
}
let owner = CommandLine.arguments[1]
let list = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
for w in list where (w[kCGWindowOwnerName as String] as? String) == owner && (w[kCGWindowLayer as String] as? Int) == 0 {
    let b = w[kCGWindowBounds as String] as? [String: Double] ?? [:]
    let fields: [String: Any] = [
        "id": w[kCGWindowNumber as String] as? Int ?? 0,
        "pid": w[kCGWindowOwnerPID as String] as? Int ?? 0,
        "title": w[kCGWindowName as String] as? String ?? "",
        "x": b["X"] ?? 0, "y": b["Y"] ?? 0, "width": b["Width"] ?? 0, "height": b["Height"] ?? 0,
        "onscreen": w[kCGWindowIsOnscreen as String] as? Bool ?? false,
    ]
    if let data = try? JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys]),
       let line = String(data: data, encoding: .utf8) {
        print(line)
    }
}
