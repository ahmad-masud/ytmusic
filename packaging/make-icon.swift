// Renders the app icon into an .iconset directory.
//
//   swift packaging/make-icon.swift <output.iconset> [source-mark.png]
//
// With a source mark, it is composited onto a white rounded-square plate —
// YouTube Music's own app-icon treatment, in macOS proportions. Without one it
// falls back to drawing a generic beamed eighth note, so the build still works
// if you delete the mark for trademark reasons.
//
// `scripts/bundle.sh` runs this and then `iconutil`. The generated .icns is
// checked in, so this only needs re-running when the artwork changes.

import AppKit
import Foundation

let arguments = CommandLine.arguments
guard arguments.count >= 2 else {
    FileHandle.standardError.write(
        "usage: make-icon.swift <output.iconset> [source-mark.png]\n".data(using: .utf8)!)
    exit(1)
}

let outputDirectory = URL(fileURLWithPath: arguments[1])
try? FileManager.default.createDirectory(at: outputDirectory, withIntermediateDirectories: true)

let mark: NSImage? = arguments.count >= 3
    ? NSImage(contentsOf: URL(fileURLWithPath: arguments[2]))
    : nil

/// How much of the plate the mark occupies. Matches the safe-zone ratio in
/// YouTube Music's own maskable PWA icon.
let markScale: CGFloat = 0.70

// MARK: - Fallback glyph

// Laid out in a 44×44 design space, matching src/icon.rs.
let design: CGFloat = 44
let headCentres: [CGPoint] = [CGPoint(x: 13.5, y: 33), CGPoint(x: 31.5, y: 29)]
let headRadii = CGSize(width: 7.5, height: 6)
let headTilt: CGFloat = -20 * .pi / 180
let stems: [CGRect] = [
    CGRect(x: 18, y: 9, width: 3, height: 24),
    CGRect(x: 36, y: 5.5, width: 3, height: 23.5),
]

func notePath() -> CGPath {
    let path = CGMutablePath()
    for centre in headCentres {
        let transform = CGAffineTransform(translationX: centre.x, y: centre.y).rotated(by: headTilt)
        let bounds = CGRect(
            x: -headRadii.width, y: -headRadii.height,
            width: headRadii.width * 2, height: headRadii.height * 2)
        path.addEllipse(in: bounds, transform: transform)
    }
    for stem in stems {
        path.addRoundedRect(in: stem, cornerWidth: 1.5, cornerHeight: 1.5)
    }
    path.move(to: CGPoint(x: 18, y: 9))
    path.addLine(to: CGPoint(x: 39, y: 5.5))
    path.addLine(to: CGPoint(x: 39, y: 12))
    path.addLine(to: CGPoint(x: 18, y: 15.5))
    path.closeSubpath()
    return path
}

func gradient(_ stops: [(CGFloat, CGFloat, CGFloat)]) -> CGGradient {
    let colors = stops.map { CGColor(red: $0.0, green: $0.1, blue: $0.2, alpha: 1) }
    return CGGradient(
        colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors as CFArray, locations: [0, 1])!
}

// MARK: - Render

func render(pixels: Int) -> Data? {
    guard
        let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
    else { return nil }

    NSGraphicsContext.saveGraphicsState()
    defer { NSGraphicsContext.restoreGraphicsState() }
    guard let graphics = NSGraphicsContext(bitmapImageRep: rep) else { return nil }
    NSGraphicsContext.current = graphics
    let context = graphics.cgContext

    let canvas = CGFloat(pixels)
    context.setAllowsAntialiasing(true)
    context.interpolationQuality = .high

    // macOS icons sit inside roughly 80% of their canvas.
    let inset = canvas * 0.0977
    let plate = CGRect(x: inset, y: inset, width: canvas - inset * 2, height: canvas - inset * 2)
    let corner = plate.width * 0.2237
    let plateShape = CGPath(
        roundedRect: plate, cornerWidth: corner, cornerHeight: corner, transform: nil)

    context.saveGState()
    context.addPath(plateShape)
    context.clip()
    if mark != nil {
        // YouTube Music's mark sits on white.
        context.drawLinearGradient(
            gradient([(1.0, 1.0, 1.0), (0.976, 0.976, 0.984)]),
            start: CGPoint(x: 0, y: plate.maxY), end: CGPoint(x: 0, y: plate.minY), options: [])
    } else {
        context.drawLinearGradient(
            gradient([(0.16, 0.17, 0.20), (0.07, 0.07, 0.09)]),
            start: CGPoint(x: 0, y: plate.maxY), end: CGPoint(x: 0, y: plate.minY), options: [])
    }
    context.restoreGState()

    // Hairline rim, so a white plate still reads against a white window.
    context.saveGState()
    context.setStrokeColor(
        mark != nil
            ? CGColor(red: 0, green: 0, blue: 0, alpha: 0.10)
            : CGColor(red: 1, green: 1, blue: 1, alpha: 0.10))
    context.setLineWidth(max(1, canvas / 256))
    context.addPath(plateShape)
    context.strokePath()
    context.restoreGState()

    if let mark {
        let side = plate.width * markScale
        let box = CGRect(
            x: plate.midX - side / 2, y: plate.midY - side / 2, width: side, height: side)
        mark.draw(in: box, from: .zero, operation: .sourceOver, fraction: 1.0)
    } else {
        context.saveGState()
        let scale = plate.width / design
        context.translateBy(x: plate.minX, y: plate.maxY)
        context.scaleBy(x: scale, y: -scale)
        context.addPath(notePath())
        context.clip()
        context.drawLinearGradient(
            gradient([(1.0, 0.42, 0.38), (0.90, 0.13, 0.22)]),
            start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: design), options: [])
        context.restoreGState()
    }

    return rep.representation(using: .png, properties: [:])
}

let variants: [(String, Int)] = [
    ("icon_16x16", 16), ("icon_16x16@2x", 32),
    ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256),
    ("icon_256x256", 256), ("icon_256x256@2x", 512),
    ("icon_512x512", 512), ("icon_512x512@2x", 1024),
]

for (name, pixels) in variants {
    guard let data = render(pixels: pixels) else {
        FileHandle.standardError.write("failed to render \(name)\n".data(using: .utf8)!)
        exit(1)
    }
    try data.write(to: outputDirectory.appendingPathComponent("\(name).png"))
}

print("wrote \(variants.count) images to \(outputDirectory.path)\(mark == nil ? " (fallback glyph)" : "")")
