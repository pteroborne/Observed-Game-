//! Retired Arc-L generator entrypoint.
//!
//! `.map` files are now canonical hand-authored sources. Keeping this binary as
//! a hard failure prevents an old command from silently overwriting a
//! TrenchBroom edit.

fn main() {
    eprintln!(
        "bake_tiles is retired: assets/tiles/*.map are canonical. \
         Use `cargo run -p observed_authoring --bin tilec -- audit` or `... -- build`."
    );
    std::process::exit(2);
}
