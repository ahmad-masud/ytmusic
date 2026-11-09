//! The menu bar glyph, rasterised at runtime.
//!
//! This is the YouTube Music mark reduced to a silhouette: the filled disc,
//! with the ring and the play triangle knocked out of it exactly as the white
//! parts of the real logo are. The menu bar takes *template* images: AppKit uses
//! only the alpha channel and recolours it for light and dark menu bars, which
//! is why this is a shape rather than the red logo. A full-colour icon up there
//! would be wrong on macOS, and illegible at 18pt besides.
//!
//! `tray-icon` wants raw RGBA, and shipping a PNG would mean pulling in an
//! image decoder for one 44×44 bitmap, so it is drawn analytically instead.

const SIZE: u32 = 44;
const SAMPLES: u32 = 4;

const CENTRE: (f64, f64) = (22.0, 22.0);
/// The filled disc.
const DISC: f64 = 19.0;
/// The ring is knocked *out* of the disc, as it is in the real mark.
const RING_GAP: (f64, f64) = (15.4, 17.2);

/// Play triangle, pointing right, also knocked out.
const TRIANGLE: [(f64, f64); 3] = [(17.5, 15.0), (17.5, 29.0), (28.5, 22.0)];

pub fn tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let alpha = (coverage(x, y) * 255.0).round().clamp(0.0, 255.0) as u8;
            // Template images ignore colour; white keeps the premultiplied
            // edges clean if that ever changes.
            rgba.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE)
}

/// Fraction of the pixel covered by the glyph, by supersampling.
fn coverage(px: u32, py: u32) -> f64 {
    let mut hits = 0u32;
    let step = 1.0 / f64::from(SAMPLES);
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let x = f64::from(px) + (f64::from(sx) + 0.5) * step;
            let y = f64::from(py) + (f64::from(sy) + 0.5) * step;
            if inside(x, y) {
                hits += 1;
            }
        }
    }
    f64::from(hits) / f64::from(SAMPLES * SAMPLES)
}

/// Ink where the mark is red: the disc, minus the ring and the play triangle.
fn inside(x: f64, y: f64) -> bool {
    let distance = (x - CENTRE.0).hypot(y - CENTRE.1);
    let in_disc = distance <= DISC;
    let in_ring_gap = distance >= RING_GAP.0 && distance <= RING_GAP.1;
    in_disc && !in_ring_gap && !in_triangle(x, y)
}

fn in_triangle(x: f64, y: f64) -> bool {
    // Same winding for all three edges means the point is inside.
    let edge = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
    let [a, b, c] = TRIANGLE;
    let (ab, bc, ca) = (edge(a, b), edge(b, c), edge(c, a));
    (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
}
