//! A fixed-input dump of every geometry primitive, frozen against the Python.
//!
//! `tools/tileforge.py` is the thing this port has to agree with, and the
//! agreement has to outlive it: once the Python is deleted there is nothing
//! left to differ against, and "it matched at the time" stops being checkable.
//!
//! So the Python's output at these exact inputs is committed as
//! `tests/fixtures/forge_primitives.txt`, and [`primitive_probe`] must
//! reproduce it byte for byte. The fixture is a frozen record of what the
//! Python produced, not a record of what this module produces - regenerating it
//! from the Rust would turn the gate into a tautology and is exactly the
//! mistake to avoid if a future change makes it fail.
//!
//! Every one of these inputs was chosen because it exercises a branch that is
//! silent when wrong: chamfered and unchamfered prisms, all six faces (so a
//! winding flip cannot hide in symmetry), a splayed door with its bevelled
//! lintel, a sloped top plane, and a textual translate.

use super::geometry as g;

/// Dump every primitive at fixed inputs.
#[must_use]
pub fn primitive_probe() -> String {
    let mut out: Vec<String> = Vec::new();

    out.push("--fmt".into());
    for v in [
        0.0,
        -0.0,
        112.0,
        -64.0,
        0.5,
        -13.8564,
        1.0 / 3.0,
        2.00005,
        -2.00005,
    ] {
        out.push(g::fmt(v));
    }

    out.push("--edge/mid/offset".into());
    for face in 0..6 {
        let (a, b) = g::edge(face);
        let m = g::face_mid(face);
        let (oa, ob) = g::offset_inward(a, b, 8.0);
        out.push(format!(
            "{} {} {} {} {}",
            pair(a),
            pair(b),
            pair(m),
            pair(oa),
            pair(ob)
        ));
    }

    out.push("--cell_origin".into());
    for (q, r) in [(0, 0), (1, 0), (0, 1), (-1, 1), (2, -1)] {
        out.push(pair(g::cell_origin(q, r)));
    }

    let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];

    out.push("--prism".into());
    out.push(g::prism(&square, 0.0, 8.0, None, 2.0, 1.0));
    out.push("--hex_slab".into());
    out.push(g::hex_slab(0.0, 8.0, 2.0, 0.0));
    out.push("--band/wall".into());
    out.push(g::band(1, 0.0, 8.0, 8.0, 128.0));
    out.push(g::wall(3, 8.0, 128.0));
    out.push("--box".into());
    out.push(g::boxed((-10.0, -10.0, 0.0), (10.0, 10.0, 8.0)));
    out.push("--sloped".into());
    out.push(g::sloped_prism(
        &square,
        0.0,
        [(0.0, 0.0, 8.0), (10.0, 0.0, 8.0), (0.0, 10.0, 20.0)],
        None,
    ));
    out.push("--translate".into());
    out.push(g::translate(
        &g::boxed((-10.0, -10.0, 0.0), (10.0, 10.0, 8.0)),
        112.0,
        -64.0,
        128.0,
    ));
    out.push("--door_wall".into());
    out.push(g::door_wall_default(0, 8.0, 128.0));
    out.push("--pylon".into());
    out.push(g::pylon(40.0, 0.0, 128.0, 30.0, 4.0, 0.0));

    out.join("\n")
}

/// Python's `repr` of a float 2-tuple, so the dumps compare directly.
///
/// Only used by the probe: the emitted `.map` format goes through
/// [`g::fmt`], which is a different and much stricter rule.
fn pair(p: (f64, f64)) -> String {
    format!("({}, {})", repr(p.0), repr(p.1))
}

fn repr(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e16 {
        return format!("{v:.1}");
    }
    let text = format!("{v}");
    if text.contains('.') || text.contains('e') {
        text
    } else {
        format!("{text}.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port must reproduce what the Python produced, byte for byte.
    ///
    /// This is the gate the whole port rests on. It is deliberately a
    /// comparison against a committed artifact rather than a self-consistency
    /// check: a test that compares the Rust to itself would pass through any
    /// amount of drift.
    #[test]
    fn the_primitives_reproduce_the_python_byte_for_byte() {
        let fixture = include_str!("../../tests/fixtures/forge_primitives.txt");
        let produced = primitive_probe();
        // Git may hand back CRLF on Windows; the contract is on content.
        let fixture = fixture.replace("\r\n", "\n");

        if fixture != produced {
            // Name the first divergence rather than dumping 6 KB of context:
            // a plane-winding bug shows up as one line and is invisible in a
            // wall of coordinates.
            let at = fixture
                .lines()
                .zip(produced.lines())
                .position(|(a, b)| a != b);
            match at {
                Some(index) => {
                    let expected: Vec<&str> = fixture.lines().collect();
                    let actual: Vec<&str> = produced.lines().collect();
                    panic!(
                        "line {} diverges from the Python:\n  python: {}\n  rust:   {}",
                        index + 1,
                        expected[index],
                        actual[index]
                    );
                }
                None => panic!(
                    "output length differs: python has {} lines, rust has {}",
                    fixture.lines().count(),
                    produced.lines().count()
                ),
            }
        }
    }

    /// The fixture has to actually contain the hard cases, or the gate above
    /// passes while checking nothing interesting.
    #[test]
    fn the_fixture_covers_the_branches_that_fail_silently() {
        let fixture = include_str!("../../tests/fixtures/forge_primitives.txt");
        for section in [
            "--fmt",
            "--edge/mid/offset",
            "--cell_origin",
            "--prism",
            "--hex_slab",
            "--band/wall",
            "--box",
            "--sloped",
            "--translate",
            "--door_wall",
            "--pylon",
        ] {
            assert!(fixture.contains(section), "fixture is missing {section}");
        }
        assert!(
            fixture.lines().filter(|l| l.contains("__TB_empty")).count() > 60,
            "the fixture should carry real brush geometry, not just headers"
        );
    }
}
