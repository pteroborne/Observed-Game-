//! The rules of a match, with none of its presentation.
//!
//! A board of hexes whose **edges** carry passability, a turn pipeline that
//! never varies, and six swappable strategies that do:
//!
//! | Seam | What it decides |
//! | --- | --- |
//! | [`resolution`] | how simultaneous intents become positions |
//! | [`vision`] | what a pawn holds by looking at it |
//! | [`threat`] | what hunts the pawns |
//! | [`setback`] | what being caught costs |
//! | [`mutation`] | what the facility does when nobody is looking |
//! | [`objective`] | what winning is |
//!
//! One rule keeps that list honest: **a trait ships with two implementations
//! or it is not a trait yet.** One implementor is a speculative abstraction;
//! two is a comparison, which is what the seams exist for.
//!
//! A match is reproducible from a [`spec::ModeSpec`], a seed and an intent log.
//! That is not a convenience — it is what lets two clients share a match by
//! exchanging a growing text log rather than replicating state, and what lets a
//! terminal harness replay a whole game to render one turn.

mod sim;

pub use sim::{
    architect, board, bot, mutation, objective, prng, resolution, rules, setback, state, step,
    threat, tiles, vision,
};

pub mod spec;
