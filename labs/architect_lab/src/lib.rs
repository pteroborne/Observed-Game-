//! One deterministic Rogue pressure simulation, with desktop and browser views.
pub use observed_match::ascent::{economy, falls, placement, prison, requisition, sim};

#[cfg(all(test, any(feature = "desktop", feature = "web")))]
mod lifecycle_tests;
pub use sim::PowerPolicy;

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub(crate) mod view;
#[cfg(feature = "desktop")]
pub(crate) use desktop::ArchitectAction;
#[cfg(feature = "desktop")]
pub use desktop::{ArchitectLabPlugin, LabSession, run};

#[cfg(feature = "web")]
pub(crate) mod web;
