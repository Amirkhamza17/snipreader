// Draws the 1024×1024 app icon: the menu-bar glyph (selection corners + two
// text lines) in white on a blue rounded square.
// Usage: swift make_icon.swift <out.png>
import AppKit

let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon_1024.png"
let size = 1024
let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

// macOS icon grid: 824×824 body centred in 1024, corner radius ≈ 185.
let body = NSRect(x: 100, y: 100, width: 824, height: 824)
let shape = NSBezierPath(roundedRect: body, xRadius: 185, yRadius: 185)
NSGraphicsContext.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(0.25)
shadow.shadowOffset = NSSize(width: 0, height: -10)
shadow.shadowBlurRadius = 24
shadow.set()
NSColor.white.setFill()
shape.fill()
NSGraphicsContext.restoreGraphicsState()
NSGradient(
    starting: NSColor(calibratedRed: 0.30, green: 0.62, blue: 1.00, alpha: 1),
    ending: NSColor(calibratedRed: 0.10, green: 0.36, blue: 0.90, alpha: 1)
)!.draw(in: shape, angle: -90)

// Glyph: same proportions as the 36-px tray icon, scaled into the body.
let unit = body.width / 36.0
func rect(_ x: Double, _ y: Double, _ w: Double, _ h: Double) -> NSRect {
    // Tray coordinates have y going down; flip into AppKit's y-up space.
    NSRect(x: body.minX + x * unit, y: body.maxY - (y + h) * unit, width: w * unit, height: h * unit)
}
NSColor.white.setFill()
let (a, b, l, t) = (6.0, 30.0, 8.0, 2.4)
for (cx, cy, dx, dy) in [(a, a, 1.0, 1.0), (b, a, -1.0, 1.0), (a, b, 1.0, -1.0), (b, b, -1.0, -1.0)] {
    let hx = dx > 0 ? cx : cx - l, hy = dy > 0 ? cy : cy - t
    NSBezierPath(roundedRect: rect(hx, hy, l, t), xRadius: unit, yRadius: unit).fill()
    let vx = dx > 0 ? cx : cx - t, vy = dy > 0 ? cy : cy - l
    NSBezierPath(roundedRect: rect(vx, vy, t, l), xRadius: unit, yRadius: unit).fill()
}
NSBezierPath(roundedRect: rect(11, 13.5, 14, 3), xRadius: 1.5 * unit, yRadius: 1.5 * unit).fill()
NSBezierPath(roundedRect: rect(11, 19.5, 9, 3), xRadius: 1.5 * unit, yRadius: 1.5 * unit).fill()

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
print("wrote \(out)")
