//! One deterministic Rogue pressure simulation, with desktop and browser views.
pub mod requisition;
pub mod sim;

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
mod view;
#[cfg(feature = "desktop")]
pub(crate) use desktop::ArchitectAction;
#[cfg(feature = "desktop")]
pub use desktop::{ArchitectLabPlugin, LabSession, run};

#[cfg(feature = "web")]
mod web;
