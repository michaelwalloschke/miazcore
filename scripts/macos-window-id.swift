import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 3,
      let pid = Int32(CommandLine.arguments[1]) else {
    fputs("usage: macos-window-id.swift <pid> <exact-title>\n", stderr)
    exit(64)
}

let title = CommandLine.arguments[2]
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    fputs("unable to enumerate macOS windows\n", stderr)
    exit(65)
}

for window in windows {
    guard window[kCGWindowOwnerPID as String] as? Int32 == pid,
          window[kCGWindowName as String] as? String == title,
          window[kCGWindowLayer as String] as? Int == 0,
          let identifier = window[kCGWindowNumber as String] as? UInt32 else { continue }
    print(identifier)
    exit(0)
}
fputs("exact Diagnostic World window was not found\n", stderr)
exit(66)
