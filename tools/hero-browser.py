#!/usr/bin/env python3
"""Drives ONE dedicated BrowserOS window for tools/record-real.sh, through BrowserOS's MCP server
(http://127.0.0.1:9000/mcp). Nothing outside that window is touched: every call names the window
or its single page.

Usage: tools/hero-browser.py <state.json> open <pair-url>
       tools/hero-browser.py <state.json> activate
       tools/hero-browser.py <state.json> layout <viewport-w> <viewport-h>
       tools/hero-browser.py <state.json> thread <url> <title>
       tools/hero-browser.py <state.json> close

* open: new window, pairs with T3's web UI through the one-time pairing URL, collapses T3's main
  sidebar (other projects and threads stay out of the window) and parks the window on a blank
  black page. Writes the window/page ids to <state.json>.
* activate: brings the window to the front (macOS switches to its Space; ScreenCaptureKit gets
  no frames from a window on another Space, and Chrome does not lay out a hidden window).
* layout: resizes the (visible) window so the page viewport is <viewport-w> x <viewport-h>
  points; writes the window bounds and the chrome size (outer - inner) to <state.json>.
* thread: shows <url> (a T3 thread) once its <title> is on the page; closes the sidebar and the
  right panel if T3 shows them.
* close: closes the window.
"""

import json
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

MCP = "http://127.0.0.1:9000/mcp"
BLANK = "data:text/html,<title>T3</title><body style='background:%23000'></body>"


class Browser:
    def __init__(self, state: Path) -> None:
        self.path = state
        self.state = json.loads(state.read_text()) if state.is_file() else {}

    def save(self) -> None:
        self.path.write_text(json.dumps(self.state, indent=1))

    def call(self, tool: str, args: dict, timeout: float = 120) -> dict:
        if self.state.get("session"):
            args = {**args, "session": self.state["session"]}
        body = {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}}
        req = urllib.request.Request(MCP, data=json.dumps(body).encode(), headers={
            "Content-Type": "application/json", "Accept": "application/json, text/event-stream"})
        raw = urllib.request.urlopen(req, timeout=timeout).read().decode()
        for line in raw.splitlines():
            if line.startswith("data:"):
                raw = line[5:]
        res = json.loads(raw)
        if "error" in res:
            raise SystemExit(f"browseros {tool}: {res['error']}")
        result = res["result"]
        self.state["session"] = result.get("_meta", {}).get("com.browseros/session", self.state.get("session"))
        if result.get("isError"):
            raise SystemExit(f"browseros {tool}: {self.text(result)}")
        return result

    @staticmethod
    def text(result: dict) -> str:
        return "\n".join(c.get("text", "") for c in result.get("content", []))

    def run(self, code: str, timeout_ms: int = 30000):
        """Runs `code` (the browser SDK) and returns its JSON `return` value."""
        out = self.text(self.call("run", {"code": f"return JSON.stringify(await (async () => {{{code}}})());",
                                          "timeout": timeout_ms}, timeout_ms / 1000 + 30))
        marker = "return: "
        if marker not in out:
            raise SystemExit(f"browseros run: {out}")
        value = out.split(marker, 1)[1].rsplit("[END_UNTRUSTED", 1)[0].strip()
        return json.loads(json.loads(value))

    def js(self, expr: str):
        page = self.state["page"]
        return self.run(f"""
            const r = await browser.cdpJsonForPage({page}, 'Runtime.evaluate',
                JSON.stringify({{expression: {json.dumps(expr)}, returnByValue: true, awaitPromise: true}}));
            return r.result.value;""")

    def nav(self, url: str) -> None:
        self.call("navigate", {"page": self.state["page"], "url": url})

    def snapshot(self) -> str:
        return self.text(self.call("snapshot", {"page": self.state["page"]}))

    def click_named(self, name: str) -> bool:
        """Clicks the button whose accessible name is exactly `name` (if any)."""
        for line in self.snapshot().splitlines():
            if f'button "{name}"' in line and "[ref=" in line:
                ref = line.split("[ref=", 1)[1].split("]", 1)[0]
                self.call("act", {"page": self.state["page"], "kind": "click", "ref": ref})
                return True
        return False

    def bounds(self, w: int, h: int) -> dict:
        win, target = self.state["window"], self.state["target"]
        return self.run(f"""
            await browser.cdp('Browser.setWindowBounds', {{windowId: {win},
                bounds: {{left: 40, top: 40, width: {w}, height: {h}, windowState: 'normal'}}}});
            await new Promise(r => setTimeout(r, 600));
            return (await browser.cdp('Browser.getWindowForTarget', {{targetId: '{target}'}})).bounds;""")

    def sidebar_hidden(self) -> None:
        # The main sidebar is the "Threads" navigation (every project's threads): keep it closed.
        for _ in range(3):
            if 'navigation "Threads"' not in self.snapshot():
                return
            self.click_named("Toggle main sidebar")
            time.sleep(0.8)
        raise SystemExit("T3's main sidebar stays open")


def cmd_open(b: Browser, pair_url: str) -> None:
    created = b.text(b.call("windows", {"action": "create"}))
    window = int(created.rsplit(" ", 1)[1])
    page = b.run(f"return (await browser.pages.list()).filter(p => p.windowId === {window});")
    if len(page) != 1:
        raise SystemExit(f"new window {window}: expected one tab, got {page}")
    b.state.update(window=window, page=page[0]["pageId"], target=page[0]["targetId"])
    b.save()
    # The pairing page reads the token from the URL fragment and pairs on load.
    b.nav(pair_url)
    for _ in range(20):
        time.sleep(0.5)
        if "/pair" not in b.js("location.pathname"):
            break
    b.nav("http://127.0.0.1:3773/")
    time.sleep(2)
    if "/pair" in b.js("location.pathname"):
        raise SystemExit("T3 still shows the pairing page: the pairing link was not accepted")
    b.sidebar_hidden()
    b.nav(BLANK)
    print(json.dumps(b.state))


def cmd_layout(b: Browser, vw: int, vh: int) -> None:
    def measure() -> list[int]:
        return json.loads(b.js("JSON.stringify([innerWidth, innerHeight, outerWidth, outerHeight])"))

    # The chrome (tab strip, toolbar, side panel) measured at a first size, then the final size.
    b.bounds(vw + 300, vh + 120)
    time.sleep(0.5)
    iw, ih, ow, oh = measure()
    final = b.bounds(vw + ow - iw, vh + oh - ih)
    time.sleep(0.5)
    iw, ih, ow, oh = measure()
    if (iw, ih) != (vw, vh):
        raise SystemExit(f"viewport {iw}x{ih}, wanted {vw}x{vh} (window {final})")
    b.state.update(bounds=final, inner=[iw, ih], outer=[ow, oh], chrome=[ow - iw, oh - ih])
    print(json.dumps(b.state))


def cmd_thread(b: Browser, url: str, title: str) -> None:
    b.nav(url)
    for _ in range(40):
        if title[:40] in b.js("document.body.innerText"):
            break
        time.sleep(0.5)
    else:
        raise SystemExit(f"thread page never showed {title!r}")
    b.sidebar_hidden()
    # The right panel names this Mac (environment picker): keep it closed.
    host = subprocess.run(["scutil", "--get", "ComputerName"], capture_output=True, text=True).stdout.strip()
    for _ in range(3):
        if not host or host not in b.js("document.body.innerText"):
            break
        b.click_named("Toggle right panel")
        time.sleep(0.8)
    else:
        raise SystemExit("T3's right panel (this Mac's name) stays open")
    print(json.dumps({"url": b.js("location.href"), "title": b.js("document.title")}))


def main() -> None:
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    b = Browser(Path(sys.argv[1]))
    cmd, args = sys.argv[2], sys.argv[3:]
    if cmd == "open":
        cmd_open(b, args[0])
    elif cmd == "layout":
        cmd_layout(b, int(args[0]), int(args[1]))
    elif cmd == "thread":
        cmd_thread(b, args[0], args[1])
    elif cmd == "activate":
        b.call("windows", {"action": "activate", "windowId": b.state["window"]})
    elif cmd == "close":
        b.call("windows", {"action": "close", "windowId": b.state["window"]})
    else:
        sys.exit(__doc__)
    b.save()


if __name__ == "__main__":
    main()
