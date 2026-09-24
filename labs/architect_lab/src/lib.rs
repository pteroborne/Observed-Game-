//! One deterministic Rogue pressure simulation, with desktop and browser views.
pub mod economy;
pub mod falls;
pub mod placement;
pub mod prison;
pub mod requisition;
pub mod sim;
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
