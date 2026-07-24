//! **Deterministic lockstep networking** as a production model.
//!
//! - [`protocol`] — the fixed, checksummed wire representation: quantized
//!   `PlayerIntent`, status packets with cumulative ACKs, and the codec both peers use
//!   to reconstruct identical inputs without platform-specific serialization.
//! - [`network`] — the lockstep model over a (simulated) hostile datagram transport:
//!   both peers simulate both bodies through the production controller, frames commit
//!   only once both inputs arrive, and resend repairs loss/delay/duplication/reorder so
//!   the peers converge on the identical state (which is also the replay tape).
//!
//! Promoted out of `network_lab` in refactor R9; the lab is the debug projection.
//! Depends on `glam`, the pure `player_input`, and the production `observed_traversal`
//! controller. The optional, default-on `bevy` feature derives `Resource` on the
//! lockstep world for the lab.

pub mod lan;
pub mod netmatch;
pub mod network;
pub mod protocol;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketError {
    WrongLength,
    WrongMagic,
    UnsupportedVersion,
    InvalidPeer,
    InvalidFlags,
    InvalidAction,
    BadChecksum,
}
