// snipreader-ocr: reads the text in an image with VisionKit's Live Text (ImageAnalyzer)
// and prints it. SnipReader runs it for each capture.
//
// Why not plain Vision: before Vision can recognize text for an app, macOS prepares its
// models for that app (30–60 s), after install and again whenever it throws them away,
// which it does after some hours. Live Text uses the models the system keeps ready for
// Photos, Safari and the rest, so even the first capture after a long break takes well
// under a second.
//
// Even so, macOS throws away the compiled Live Text model when it goes unused for a while
// (overnight, say), and the next request rebuilds it: ~30 s. SnipReader runs this helper
// with --warm after a wake and every half hour, so that rebuild happens in the background.
//
// Usage: snipreader-ocr <image> [language, e.g. en-US] [auto-detect: 0|1]
//        snipreader-ocr --warm [language] [auto-detect: 0|1]   (reads a sample text)
// Exit codes: 0 = text on stdout (may be empty), 1 = error on stderr,
//             3 = Live Text not available on this Mac (the app then uses Vision).
import AppKit
import VisionKit

func fail(_ message: String, _ code: Int32) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(code)
}

// This tool lives inside SnipReader.app, so macOS takes it for the app itself when VisionKit
// connects to the window server. Info.plist's LSUIElement keeps it out of the Dock (otherwise
// the app's icon would bounce for as long as the helper runs); this keeps it from ever
// becoming the active app.
NSApplication.shared.setActivationPolicy(.prohibited)

let args = CommandLine.arguments
guard args.count >= 2 else { fail("usage: snipreader-ocr <image>|--warm [language] [auto-detect 0|1]", 1) }
let path = args[1]
let language = args.count > 2 ? args[2] : ""
let autoDetect = args.count > 3 && args[3] == "1"

guard #available(macOS 13.0, *), ImageAnalyzer.isSupported else {
    fail("Live Text is not available on this Mac", 3)
}

/// A small image with a line of text, drawn here (no window needed).
func sampleText() -> NSImage? {
    let size = NSSize(width: 360, height: 60)
    guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width), pixelsHigh: Int(size.height),
                                     bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                     colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
          let ctx = NSGraphicsContext(bitmapImageRep: rep) else { return nil }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = ctx
    NSColor.white.setFill()
    NSRect(origin: .zero, size: size).fill()
    ("Text Recognition 2026" as NSString).draw(at: NSPoint(x: 12, y: 14), withAttributes: [
        .font: NSFont.systemFont(ofSize: 28), .foregroundColor: NSColor.black,
    ])
    NSGraphicsContext.restoreGraphicsState()
    let image = NSImage(size: size)
    image.addRepresentation(rep)
    return image
}

let loaded = path == "--warm" ? sampleText() : NSImage(contentsOfFile: path)
guard let image = loaded else { fail("Cannot open the image", 1) }

var result: Result<String, Error>?
Task { @MainActor in
    do {
        var config = ImageAnalyzer.Configuration([.text])
        // A chosen language guides recognition; with auto-detect Live Text picks itself.
        if !autoDetect, !language.isEmpty {
            config.locales = [language]
        }
        let analysis = try await ImageAnalyzer().analyze(image, orientation: .up, configuration: config)
        result = .success(analysis.transcript)
    } catch {
        result = .failure(error)
    }
}
// ImageAnalyzer delivers on the main actor: keep the main run loop turning until it does.
while result == nil {
    RunLoop.main.run(until: Date().addingTimeInterval(0.01))
}
switch result! {
case .success(let text):
    print(text, terminator: "")
case .failure(let error):
    fail(error.localizedDescription, 1)
}
