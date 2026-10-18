// mouse <x> <y> [click|move] [scroll-lines]: move the pointer to screen point (x, y); `click` also clicks there,
// and a number after `click` or `move` scrolls that many lines (negative scrolls down).
// Posting CGEvents needs no Accessibility permission. The moved event and the pauses matter: a bare
// down/up pair without them is often ignored by the webview.
import CoreGraphics
import Foundation

let a = CommandLine.arguments
guard a.count >= 3, let x = Double(a[1]), let y = Double(a[2]) else {
    print("usage: mouse <x> <y> [click|move] [scroll-lines]"); exit(64)
}
let p = CGPoint(x: x, y: y)
let src = CGEventSource(stateID: .hidSystemState)
CGEvent(mouseEventSource: src, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
usleep(150_000)
if a.count > 3 && a[3] == "click" {
    CGEvent(mouseEventSource: src, mouseType: .leftMouseDown, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
    usleep(80_000)
    CGEvent(mouseEventSource: src, mouseType: .leftMouseUp, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
}
if a.count > 4, let lines = Int32(a[4]) {
    CGEvent(scrollWheelEvent2Source: src, units: .line, wheelCount: 1, wheel1: lines, wheel2: 0, wheel3: 0)?.post(tap: .cghidEventTap)
}
