//! District-specific construction against sealed bays, with a clear central aisle.
use super::{InitialHallKind, trim};
use crate::forge::geometry::{FLOOR_TOP, band};

pub(super) fn bay(register: &str, kind: InitialHallKind, face: usize, terrace: bool) -> String {
    let mut out = String::new();
    match register {
        "shadow_screen" => {
            if kind == InitialHallKind::Gallery {
                // A wall-backed slatted screen, with genuinely separate rails.
                // It stays outside every centre-to-door approach.
                for height in [24.0, 40.0, 56.0, 72.0] {
                    out.push_str(&trim(band(face, 8.0, 18.0, height, height + 4.0)));
                }
            } else {
                // Empty garden ledges give the taller court a quiet low datum.
                out.push_str(&band(face, 8.0, 22.0, FLOOR_TOP, 15.2));
                out.push_str(&trim(band(face, 7.0, 23.0, 15.2, 17.2)));
            }
        }
        "facet_monument" => {
            if terrace {
                // The parapet is the guard; the shallow band is its coping.
                out.push_str(&trim(band(face, 0.0, 9.0, 24.0, 26.0)));
            } else if kind == InitialHallKind::Gallery {
                out.push_str(&band(face, 8.0, 20.0, FLOOR_TOP, 24.0));
                out.push_str(&trim(band(face, 7.0, 21.0, 24.0, 26.0)));
            } else {
                // Three massive wall courses contrast with the lower gallery.
                for height in [40.0, 72.0, 104.0] {
                    out.push_str(&trim(band(face, 8.0, 16.0, height, height + 6.0)));
                }
            }
        }
        "megastructure" => {
            // Service banks stay backed by sealed walls and outside the aisle.
            let top = if kind == InitialHallKind::Gallery {
                36.0
            } else {
                48.0
            };
            out.push_str(&band(face, 8.0, 26.0, FLOOR_TOP, top));
            out.push_str(&trim(band(face, 7.0, 27.0, top, top + 3.0)));
            let courses: &[f64] = if kind == InitialHallKind::Gallery {
                &[48.0, 64.0]
            } else {
                &[72.0, 104.0]
            };
            for &height in courses {
                out.push_str(&trim(band(face, 8.0, 18.0, height, height + 6.0)));
            }
        }
        "thinning" => {
            // A wide coping makes the open perimeter read as a deliberate guard.
            out.push_str(&trim(band(face, 0.0, 12.0, 24.0, 27.0)));
        }
        _ => {}
    }
    out
}
