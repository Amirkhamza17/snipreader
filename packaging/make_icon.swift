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
// Selection corners: each one a single stroke with a rounded bend and round ends.
func pt(_ x: Double, _ y: Double) -> NSPoint {
    NSPoint(x: body.minX + x * unit, y: body.maxY - y * unit)
}
let (a, b, l, t, bend) = (7.2, 28.8, 7.0, 2.4, 3.2)
NSColor.white.setStroke()
for (cx, cy, dx, dy) in [(a, a, 1.0, 1.0), (b, a, -1.0, 1.0), (a, b, 1.0, -1.0), (b, b, -1.0, -1.0)] {
    let corner = NSBezierPath()
    corner.lineWidth = t * unit
    corner.lineCapStyle = .round
    corner.lineJoinStyle = .round
    corner.move(to: pt(cx + dx * l, cy))                 // end of the horizontal arm
    corner.line(to: pt(cx + dx * bend, cy))
    corner.curve(to: pt(cx, cy + dy * bend),             // the rounded bend
                 controlPoint1: pt(cx + dx * bend * 0.45, cy),
                 controlPoint2: pt(cx, cy + dy * bend * 0.45))
    corner.line(to: pt(cx, cy + dy * l))                 // end of the vertical arm
    corner.stroke()
}
NSBezierPath(roundedRect: rect(11, 13.5, 14, 3), xRadius: 1.5 * unit, yRadius: 1.5 * unit).fill()
NSBezierPath(roundedRect: rect(11, 19.5, 9, 3), xRadius: 1.5 * unit, yRadius: 1.5 * unit).fill()

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
print("wrote \(out)")
