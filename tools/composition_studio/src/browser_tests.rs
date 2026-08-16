//! Source gates for the browser build.
//!
//! The studio ships as a hosted wasm viewer as well as a desktop binary, and
//! the browser is unforgiving in ways the compiler cannot see: an API that
//! merely panics at run time on `wasm32-unknown-unknown` compiles perfectly.
//! These are ratchets against the ones that have actually bitten.
//!
//! Split out rather than added to `tests.rs` for the same reason the other test
//! modules are: that file sits at the 600-line review budget.

use crate::tests::studio_sources;

/// `std::time` is a trapdoor in a build that also targets the browser.
///
/// `SystemTime::now` and `Instant::now` are not implemented on
/// wasm32-unknown-unknown - they panic with "time not implemented on this
/// platform". A panic there aborts the app, so the studio boots, acquires a
/// WebGL2 adapter, logs nothing alarming, and then renders a black canvas
/// forever. From the outside that is indistinguishable from a slow load, which
/// is why it survived several rounds of confident diagnosis.
///
/// `bevy::platform::time::Instant` and `web_time::SystemTime` are the portable
/// spellings, and both are `std` when built natively.
///
/// Two exemptions, both because the code cannot reach a browser: `module/`
/// belongs to the desktop-only `module-studio` binary, and `#[cfg(test)]`
/// modules are never compiled into the wasm artifact.
#[test]
fn the_browser_path_never_reads_the_clock_through_std_time() {
    // Assembled rather than written out, so this gate does not trip over its
    // own source text.
    let banned = [
        format!("std::time{}", "::SystemTime"),
        format!("std::time{}", "::Instant"),
    ];

    let mut checked = 0;
    for (path, text) in studio_sources() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.ends_with("tests.rs") || path.components().any(|part| part.as_os_str() == "module")
        {
            continue;
        }
        checked += 1;
        for (number, line) in text.lines().enumerate() {
            // Prose is exempt: several call sites explain in a comment exactly
            // which spelling they are avoiding, and why.
            let code = line.split("//").next().unwrap_or_default();
            for needle in &banned {
                assert!(
                    !code.contains(needle.as_str()),
                    "{}:{} reaches for {needle}, which panics in the browser; \
                     use bevy::platform::time::Instant or web_time::SystemTime",
                    path.display(),
                    number + 1,
                );
            }
        }
    }
    assert!(
        checked >= 20,
        "expected to inspect the studio's own sources; only {checked} files survived the filter"
    );
}
