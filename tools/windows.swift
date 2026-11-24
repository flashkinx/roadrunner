// Listet die Fenster von Roadrunner: ID, Ebene, Groesse, Titel.
// Fuer Screenshots: screencapture -l <ID> datei.png
import CoreGraphics
import Foundation

let opts: CGWindowListOption = [.optionOnScreenOnly]
guard let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in list {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    guard owner.lowercased().contains("roadrunner") else { continue }
    let id = w[kCGWindowNumber as String] as? Int ?? 0
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let name = w[kCGWindowName as String] as? String ?? ""
    print("\(id)\t\(layer)\t\(b["X"] ?? 0),\(b["Y"] ?? 0)\t\(b["Width"] ?? 0)x\(b["Height"] ?? 0)\t\(name)")
}
