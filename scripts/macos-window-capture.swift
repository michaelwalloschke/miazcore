import AppKit
import Foundation
import ScreenCaptureKit

guard CommandLine.arguments.count == 4,
      let pid = Int32(CommandLine.arguments[1]) else {
    fputs("usage: macos-window-capture.swift <pid> <exact-title> <output.png>\n", stderr)
    exit(64)
}

let title = CommandLine.arguments[2]
let output = URL(fileURLWithPath: CommandLine.arguments[3])
let application = NSApplication.shared
application.setActivationPolicy(.accessory)

@available(macOS 14.0, *)
func captureExactWindow() async throws {
    let content = try await SCShareableContent.excludingDesktopWindows(
        false,
        onScreenWindowsOnly: false
    )
    guard let window = content.windows.first(where: {
        $0.owningApplication?.processID == pid && $0.title == title
    }) else {
        throw CaptureError.exactWindowNotFound
    }

    let filter = SCContentFilter(desktopIndependentWindow: window)
    let configuration = SCStreamConfiguration()
    configuration.width = max(1, Int(window.frame.width.rounded(.up)))
    configuration.height = max(1, Int(window.frame.height.rounded(.up)))
    configuration.showsCursor = false

    let image = try await SCScreenshotManager.captureImage(
        contentFilter: filter,
        configuration: configuration
    )
    guard let png = NSBitmapImageRep(cgImage: image).representation(
        using: .png,
        properties: [:]
    ) else {
        throw CaptureError.pngEncodingFailed
    }
    try png.write(to: output, options: .atomic)
}

enum CaptureError: LocalizedError {
    case exactWindowNotFound
    case pngEncodingFailed

    var errorDescription: String? {
        switch self {
        case .exactWindowNotFound:
            "exact Diagnostic World window was not found"
        case .pngEncodingFailed:
            "exact Diagnostic World window could not be encoded as PNG"
        }
    }
}

if #available(macOS 14.0, *) {
    Task { @MainActor in
        do {
            try await captureExactWindow()
            exit(0)
        } catch {
            fputs("\(error.localizedDescription)\n", stderr)
            exit(1)
        }
    }
    RunLoop.main.run()
}

fputs("ScreenCaptureKit requires macOS 14 or later\n", stderr)
exit(69)
