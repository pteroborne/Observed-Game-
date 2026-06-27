//! trenchbroom_lab — Phase A1 of the asset-integration roadmap.
//!
//! Proves that an authored TrenchBroom `.map` can become this game's
//! room/corridor/door topology *without editor entities becoming the game model*.
//! `bevy_trenchbroom 0.13` is used as the **importer/parser** only: it parses the
//! authored map into brush geometry + entity properties, which [`project`] turns
//! into stable [`observed_core`] IDs and collision. Presentation is a projection of
//! that domain model, rendered through `observed_style` (so the neon-noir Legibility
//! Contract holds and map materials never introduce ad-hoc colours), and the player
//! walks the imported collision with the shared `observed_traversal` controller.

pub mod lab;
pub mod map_source;
pub mod project;

pub use lab::run;
