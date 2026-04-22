import CoreGraphics
import Foundation

// Find the widget-gallery window
func findGalleryWindow() -> (CGWindowID, CGRect)? {
    let windowList = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
    for w in windowList {
        let owner = w[kCGWindowOwnerName as String] as? String ?? ""
        let layer = w[kCGWindowLayer as String] as? Int ?? -1
        let bounds = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
        let width = bounds["Width"] as? Double ?? 0
        let height = bounds["Height"] as? Double ?? 0
        if owner == "widget-gallery" && layer == 0 && width > 100 && height > 100,
           let number = w[kCGWindowNumber as String] as? Int {
            let x = bounds["X"] as? Double ?? 0
            let y = bounds["Y"] as? Double ?? 0
            return (CGWindowID(number), CGRect(x: x, y: y, width: width, height: height))
        }
    }
    return nil
}

func clickAt(_ x: Double, _ y: Double) {
    let down = CGEvent(mouseEventSource: nil, mouseType: .leftMouseDown,
        mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)
    down?.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.05)
    let up = CGEvent(mouseEventSource: nil, mouseType: .leftMouseUp,
        mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)
    up?.post(tap: .cghidEventTap)
}

func scrollDown(at x: Double, _ y: Double, pixels: Int32) {
    // Move mouse first
    let move = CGEvent(mouseEventSource: nil, mouseType: .mouseMoved,
        mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)
    move?.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.05)

    let scroll = CGEvent(scrollWheelEvent2Source: nil, units: .pixel,
        wheelCount: 1, wheel1: pixels, wheel2: 0, wheel3: 0)
    scroll?.post(tap: .cghidEventTap)
}

func captureFullScreen(to path: String) {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    task.arguments = ["-x", path]
    try? task.run()
    task.waitUntilExit()
}

// Main
guard let (windowID, rect) = findGalleryWindow() else {
    print("ERROR: widget-gallery window not found")
    exit(1)
}

let cx = rect.midX
let cy = rect.midY
let dir = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp"

print("Found window at \(rect), center=\(cx),\(cy)")

// Click to focus
clickAt(cx, cy)
Thread.sleep(forTimeInterval: 0.5)

// Scroll to top first
for _ in 0..<20 {
    scrollDown(at: cx, cy, pixels: 200)
    Thread.sleep(forTimeInterval: 0.02)
}
Thread.sleep(forTimeInterval: 0.5)

// Capture pages
for i in 0..<8 {
    Thread.sleep(forTimeInterval: 0.3)
    captureFullScreen(to: "\(dir)/gallery_page\(i).png")
    print("Captured page \(i)")

    // Scroll down by roughly one viewport
    for _ in 0..<12 {
        scrollDown(at: cx, cy, pixels: -40)
        Thread.sleep(forTimeInterval: 0.02)
    }
    Thread.sleep(forTimeInterval: 0.3)
}

print("Done - captured 8 pages")
