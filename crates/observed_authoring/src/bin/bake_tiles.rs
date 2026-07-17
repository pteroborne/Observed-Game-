//! Regenerate the committed tile assets from the typed source.
//! Run from the workspace root: `cargo run -p observed_authoring --bin bake_tiles`

fn main() {
    let dir = std::path::Path::new("assets/tiles");
    observed_authoring::tile_source::materialize(dir).expect("tile assets must be writable");
    for (name, _) in observed_authoring::tile_source::sources() {
        println!("baked assets/tiles/{name}");
    }
}
