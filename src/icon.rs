//! The tray and window icons, rasterised at runtime.
//!
//! This is the YouTube Music mark reduced to its geometry: a filled disc with
//! the ring and the play triangle knocked out of it, exactly where the white
//! parts of the real logo are.
//!
//! It is drawn rather than decoded because shipping a PNG would mean pulling in
//! an image decoder for one small bitmap. The two platforms want different
//! things from it:
//!
//!   * macOS menu bar takes *template* images — only the alpha channel is used
//!     and AppKit recolours it per theme, so the knockouts are transparent.
//!   * Windows notification area and taskbar take full-colour icons, so the
//!     knockouts are painted white and the disc is YouTube red.

const TRAY_SIZE: u32 = 44;
#[cfg(target_os = "windows")]
const WINDOW_SIZE: u32 = 64;
const SAMPLES: u32 = 4;

const CENTRE: (f64, f64) = (0.5, 0.5);
const DISC: f64 = 19.0 / 44.0;
const RING_GAP: (f64, f64) = (15.4 / 44.0, 17.2 / 44.0);
const TRIANGLE: [(f64, f64); 3] = [
    (17.5 / 44.0, 15.0 / 44.0),
    (17.5 / 44.0, 29.0 / 44.0),
    (28.5 / 44.0, 22.0 / 44.0),
];

const RED: [f64; 3] = [255.0, 0.0, 51.0];
const WHITE: [f64; 3] = [255.0, 255.0, 255.0];

/// The menu bar / notification area icon.
pub fn tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    let colored = !cfg!(target_os = "macos");
    tray_icon::Icon::from_rgba(render(TRAY_SIZE, colored), TRAY_SIZE, TRAY_SIZE)
}

/// The title bar / taskbar / Alt-Tab icon. macOS takes this from the bundle's
/// .icns instead, so it is only used on Windows.
#[cfg(target_os = "windows")]
pub fn window_icon() -> Option<tao::window::Icon> {
    tao::window::Icon::from_rgba(render(WINDOW_SIZE, true), WINDOW_SIZE, WINDOW_SIZE).ok()
}

/// Supersampled RGBA. Coordinates are normalised so one description serves
/// every size.
fn render(size: u32, colored: bool) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let step = 1.0 / f64::from(SAMPLES);

    for y in 0..size {
        for x in 0..size {
            let mut color = [0.0f64; 3];
            let mut alpha = 0.0f64;
            let mut covered = 0.0f64;

            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = (f64::from(x) + (f64::from(sx) + 0.5) * step) / f64::from(size);
                    let py = (f64::from(y) + (f64::from(sy) + 0.5) * step) / f64::from(size);

                    let distance = (px - CENTRE.0).hypot(py - CENTRE.1);
                    if distance > DISC {
                        continue;
                    }
                    let knocked_out = (distance >= RING_GAP.0 && distance <= RING_GAP.1)
                        || in_triangle(px, py);

                    let sample = if colored {
                        // The whole disc is opaque; the knockouts are white.
                        if knocked_out { WHITE } else { RED }
                    } else if knocked_out {
                        // Template image: knockouts are holes.
                        continue;
                    } else {
                        WHITE
                    };

                    for channel in 0..3 {
                        color[channel] += sample[channel];
                    }
                    alpha += 255.0;
                    covered += 1.0;
                }
            }

            let total = f64::from(SAMPLES * SAMPLES);
            let (r, g, b) = if covered > 0.0 {
                (
                    color[0] / covered,
                    color[1] / covered,
                    color[2] / covered,
                )
            } else {
                (255.0, 255.0, 255.0)
            };
            rgba.extend_from_slice(&[
                r.round() as u8,
                g.round() as u8,
                b.round() as u8,
                (alpha / total).round() as u8,
            ]);
        }
    }
    rgba
}

fn in_triangle(x: f64, y: f64) -> bool {
    // Same winding for all three edges means the point is inside.
    let edge = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
    let [a, b, c] = TRIANGLE;
    let (ab, bc, ca) = (edge(a, b), edge(b, c), edge(c, a));
    (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
}
