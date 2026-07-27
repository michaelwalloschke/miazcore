import AppKit
import CoreImage
import CoreMedia
import CoreVideo
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

enum CaptureError: LocalizedError {
    case exactWindowNotFound
    case noCompositedSceneFrame
    case pngEncodingFailed

    var errorDescription: String? {
        switch self {
        case .exactWindowNotFound:
            "exact Diagnostic World window was not found"
        case .noCompositedSceneFrame:
            "ScreenCaptureKit did not deliver a composited Diagnostic World frame"
        case .pngEncodingFailed:
            "exact Diagnostic World window could not be encoded as PNG"
        }
    }
}

@available(macOS 14.0, *)
final class CompositedFrameOutput: NSObject, SCStreamOutput {
    private let context = CIContext()
    private let lock = NSLock()
    private var continuation: CheckedContinuation<CGImage, Error>?

    func waitForCompositedFrame() async throws -> CGImage {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            self.continuation = continuation
            lock.unlock()
        }
    }

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard outputType == .screen,
              let pixelBuffer = sampleBuffer.imageBuffer,
              let image = context.createCGImage(CIImage(cvPixelBuffer: pixelBuffer), from: CIImage(cvPixelBuffer: pixelBuffer).extent),
              hasSceneDetail(image) else { return }
        lock.lock()
        let continuation = self.continuation
        self.continuation = nil
        lock.unlock()
        continuation?.resume(returning: image)
    }

    func fail() {
        lock.lock()
        let continuation = self.continuation
        self.continuation = nil
        lock.unlock()
        continuation?.resume(throwing: CaptureError.noCompositedSceneFrame)
    }

    // Ignore the title bar and require visible colour variation in the window
    // content. A ScreenCaptureKit frame containing only the dark window chrome
    // must not acknowledge the client proof.
    private func hasSceneDetail(_ image: CGImage) -> Bool {
        guard image.bitsPerPixel == 32,
              let bytes = image.dataProvider?.data,
              let base = CFDataGetBytePtr(bytes) else { return false }
        let startX = image.width / 10
        let endX = image.width * 9 / 10
        let startY = image.height / 6
        let endY = image.height * 9 / 10
        let stepX = max(1, (endX - startX) / 40)
        let stepY = max(1, (endY - startY) / 30)
        var minimum = UInt8.max
        var maximum = UInt8.min
        for y in stride(from: startY, to: endY, by: stepY) {
            for x in stride(from: startX, to: endX, by: stepX) {
                let offset = y * image.bytesPerRow + x * 4
                let value = max(base[offset], base[offset + 1], base[offset + 2])
                minimum = min(minimum, value)
                maximum = max(maximum, value)
            }
        }
        return maximum - minimum >= 24
    }
}

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
    configuration.minimumFrameInterval = CMTime(value: 1, timescale: 30)
    configuration.queueDepth = 8
    configuration.showsCursor = false
    let frameOutput = CompositedFrameOutput()
    let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
    try stream.addStreamOutput(frameOutput, type: .screen, sampleHandlerQueue: .global(qos: .userInitiated))
    try await stream.startCapture()
    defer { Task { try? await stream.stopCapture() } }

    let image = try await withThrowingTaskGroup(of: CGImage.self) { group in
        group.addTask { try await frameOutput.waitForCompositedFrame() }
        group.addTask {
            try await Task.sleep(for: .seconds(8))
            frameOutput.fail()
            throw CaptureError.noCompositedSceneFrame
        }
        let image = try await group.next()!
        group.cancelAll()
        return image
    }
    guard let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
        throw CaptureError.pngEncodingFailed
    }
    try png.write(to: output, options: .atomic)
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
