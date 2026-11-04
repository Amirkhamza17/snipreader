// snipreader-ocr: reads the text in an image with VisionKit's Live Text (ImageAnalyzer)
// and prints it. SnipReader runs it for each capture.
//
// Why not plain Vision: before Vision can recognize text for an app, macOS prepares its
// models for that app (30–60 s), after install and again whenever it throws them away,
// which it does after some hours. Live Text uses the models the system keeps ready for
// Photos, Safari and the rest, so even the first capture after a long break takes well
// under a second.
//
// Usage: snipreader-ocr <image> [language, e.g. en-US] [auto-detect: 0|1]
// Exit codes: 0 = text on stdout (may be empty), 1 = error on stderr,
//             3 = Live Text not available on this Mac (the app then uses Vision).
import AppKit
import VisionKit

func fail(_ message: String, _ code: Int32) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(code)
}

let args = CommandLine.arguments
guard args.count >= 2 else { fail("usage: snipreader-ocr <image> [language] [auto-detect 0|1]", 1) }
let path = args[1]
let language = args.count > 2 ? args[2] : ""
let autoDetect = args.count > 3 && args[3] == "1"

guard #available(macOS 13.0, *), ImageAnalyzer.isSupported else {
    fail("Live Text is not available on this Mac", 3)
}
guard let image = NSImage(contentsOfFile: path) else { fail("Cannot open the image", 1) }

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
