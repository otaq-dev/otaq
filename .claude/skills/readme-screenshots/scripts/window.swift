// window: print "<id> <x> <y> <width> <height>" of the Otaq main window (the largest otaq-app window),
// or exit 1 if there is none. Coordinates are screen points; screenshots of a window on a 1x display are the same size.
// Also: `window --access` prints whether this process may record the screen, and asks once if not.
import CoreGraphics
import Foundation

if CommandLine.arguments.count > 1 && CommandLine.arguments[1] == "--access" {
    if CGPreflightScreenCaptureAccess() { print("screen recording: allowed"); exit(0) }
    _ = CGRequestScreenCaptureAccess()
    print("screen recording: NOT allowed (macOS just asked; allow it, then retry)"); exit(1)
}

let list = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as? [[String: Any]] ?? []
var best: (Int, CGRect)? = nil
for w in list {
    guard let owner = w["kCGWindowOwnerName"] as? String, owner == "otaq-app",
          let id = w["kCGWindowNumber"] as? Int,
          let b = w["kCGWindowBounds"] as? [String: Any],
          let rect = CGRect(dictionaryRepresentation: b as CFDictionary) else { continue }
    // Skip the menu-bar strips (30 px high) and other helper windows.
    if rect.height < 300 { continue }
    if best == nil || rect.width * rect.height > best!.1.width * best!.1.height { best = (id, rect) }
}
guard let (id, r) = best else { exit(1) }
print(id, Int(r.minX), Int(r.minY), Int(r.width), Int(r.height))
