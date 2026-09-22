//! The two questions, answered with numbers.

use crate::{Site, Space};
use std::collections::BTreeMap;

/// A tower with an atrium cut through it: the shape that makes both questions sharp.
///
/// Levels of floor, a column of pylons up one side, and a hollow core of open air. The
/// atrium is what an Observer can see across but not cross, and the pylons are what the
/// upper decks are standing on.
#[must_use]
pub fn tower_with_an_atrium(width: i32, depth: i32, levels: i32) -> Site {
    let mut site = Site::new(width, depth, levels);
    for level in 0..levels {
        for x in 0..width {
            for y in 0..depth {
                // A hollow core, two cells wide, open from the ground to the roof.
                let in_atrium = (2..width - 2).contains(&x) && (1..depth - 1).contains(&y);
                let space = if in_atrium {
                    Space::Air
                } else if x == 0 || x == width - 1 {
                    // The outer walls carry the load.
                    Space::Pylon
                } else {
                    Space::Floor
                };
                site.set((x, y, level), space);
            }
        }
    }
    site
}

/// A deck reaching out over the air, with nothing under its far end.
///
/// The Bespin case: a platform that only stands because something carries it, and which
/// a single retraction should be able to drop.
#[must_use]
pub fn suspended_deck(width: i32, depth: i32, levels: i32) -> Site {
    let mut site = Site::new(width, depth, levels);
    for level in 0..levels {
        for y in 0..depth {
            site.set((0, y, level), Space::Pylon);
        }
    }
    // A deck on the top level only, reaching out from the column across open air.
    let top = levels - 1;
    for x in 1..width {
        for y in 0..depth {
            site.set((x, y, top), Space::Floor);
        }
    }
    site
}

/// Question 1: protected cells, old rule against new.
#[must_use]
pub fn sight_comparison(site: &Site, range: i32) -> (usize, usize) {
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut old_total = 0usize;
    let mut new_total = 0usize;
    for at in site.solids() {
        for direction in directions {
            old_total += site.step_sightline(at, direction).len();
            new_total += site.sightline(at, direction, range).len();
        }
    }
    (old_total, new_total)
}

/// Question 2: how much comes down when one support goes.
#[must_use]
pub fn cascade_sizes(site: &Site) -> BTreeMap<usize, usize> {
    let mut histogram = BTreeMap::new();
    for at in site.solids() {
        let mut probe = site.clone();
        let fell = probe.retract(at);
        *histogram.entry(fell.len()).or_default() += 1;
    }
    histogram
}
