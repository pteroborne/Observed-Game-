//! Presentation-side modules: the shared UI theme, the match's drop-in asset registry,
//! and the presentation components/resources the renderer and HUD systems drive.
//!
//! Direction rule (Arc G): `view` may read `crate::sim` and the pure domain crates; it
//! never writes simulation state, and `sim` never imports `view`.

pub(crate) mod assets;
pub(crate) mod components;
pub(crate) mod sprites;
pub(crate) mod theme;
