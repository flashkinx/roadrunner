// Macht reines Schwarz, das von den Bildecken aus erreichbar ist, durchsichtig.
// PrintWindow liefert die durchsichtigen Ecken eines randlosen Fensters schwarz.
// Aufruf: swift clear-corners.swift ein.png aus.png
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let a = CommandLine.arguments
guard a.count == 3, let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[1]) as CFURL, nil),
      let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { exit(1) }
let w = img.width, h = img.height
var px = [UInt8](repeating: 0, count: w * h * 4)
let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
func dark(_ i: Int) -> Bool { px[i] < 12 && px[i + 1] < 12 && px[i + 2] < 12 && px[i + 3] > 0 }
var stack = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]
while let (x, y) = stack.popLast() {
    guard x >= 0, y >= 0, x < w, y < h else { continue }
    let i = (y * w + x) * 4
    guard dark(i) else { continue }
    px[i] = 0; px[i + 1] = 0; px[i + 2] = 0; px[i + 3] = 0
    stack += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
}
let out = ctx.makeImage()!
let dst = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[2]) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dst, out, nil)
CGImageDestinationFinalize(dst)
