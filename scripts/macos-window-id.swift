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
    // CoreGraphics bridges these numeric fields as NSNumber. Direct `as? Int`
    // or `as? Int32` casts are not stable across Swift/Foundation runtimes.
    guard let ownerPID = window[kCGWindowOwnerPID as String] as? NSNumber,
          ownerPID.int32Value == pid,
          window[kCGWindowName as String] as? String == title,
          let layer = window[kCGWindowLayer as String] as? NSNumber,
          layer.intValue == 0,
          let identifier = window[kCGWindowNumber as String] as? NSNumber else { continue }
    print(identifier.uint32Value)
    exit(0)
}
fputs("exact Diagnostic World window was not found\n", stderr)
exit(66)
