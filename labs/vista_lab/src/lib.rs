//! Vista Lab — what the facility looks like from inside open air.
//!
//! The facility gained [`HexSpace::Air`](observed_facility::hex_wfc::HexSpace::Air):
//! sight crosses it and bodies fall through it. The Architect's cutaway already
//! draws it as a well under a floating deck. This lab asks the first-person half of
//! the question: standing on a deck edge, does open air read as *height* — sheer
//! faces, hanging structures, walkways with nothing either side — while every drop
//! stays legible?
//!
//! The pure core (composition, exposure, geometry, walk) has no rendering in it and
//! is what the tests exercise; `view` draws what it says.
pub mod capture;
pub mod composition;
pub mod exposure;
pub mod geometry;
pub mod view;
pub mod walk;

#[cfg(test)]
mod tests;
