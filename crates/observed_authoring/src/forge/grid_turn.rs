//! Exact turns on the quantized hex lattice, shared by authored wonders.

use super::geometry::{P2, fmt};

/// Clockwise lattice turn in the editor's x/y plane (y is negative world z).
/// This maps every integer hex corner to the next corner, with no scale drift.
pub(super) fn turn_plan(mut point: P2, turn: u8) -> P2 {
    for _ in 0..turn % 6 {
        point = (
            point.0 * 0.5 + point.1 * 0.875,
            -point.0 * 6.0 / 7.0 + point.1 * 0.5,
        );
    }
    point
}

/// Transform the three coordinate triples on each forge-emitted brush plane.
/// Point entities are emitted separately in the same oriented frame.
pub(super) fn turn_brushes(brushes: &str, turn: u8) -> String {
    if turn == 0 {
        return brushes.to_string();
    }
    let mut out = String::new();
    for line in brushes.lines() {
        if line.starts_with("( ") {
            let mut parts: Vec<_> = line.split_whitespace().map(str::to_string).collect();
            for index in [1, 6, 11] {
                let point = (
                    parts[index].parse().expect("forge x coordinate"),
                    parts[index + 1].parse().expect("forge y coordinate"),
                );
                let turned = turn_plan(point, turn);
                parts[index] = fmt(turned.0);
                parts[index + 1] = fmt(turned.1);
            }
            out.push_str(&parts.join(" "));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}
