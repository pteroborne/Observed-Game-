//! Immutable launch inputs and retry identities shared by every entry path.

use bevy::prelude::Resource;
use observed_core::PlayerId;

use crate::play_setup::{LaunchContext, PlayRules, PlaySeat};

use super::super::launch::HexLaunchSpec;

/// Stable identity for one preparation attempt. Retry always receives a fresh value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HexLaunchRequestId(u64);

impl HexLaunchRequestId {
    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic request-id allocator shared by local, rematch, and LAN launch paths.
#[derive(Resource, Debug, Default)]
pub(crate) struct HexLaunchRequestSequence {
    last_issued: u64,
}

impl HexLaunchRequestSequence {
    /// Finalize a launch request. Callers must insert the returned resource before
    /// requesting [`crate::GameState::Loading`].
    pub(crate) fn issue(
        &mut self,
        context: LaunchContext,
        local_player: PlayerId,
        spectator: bool,
        networked: bool,
        spec: HexLaunchSpec,
        perspective: (PlayRules, PlaySeat),
    ) -> HexLaunchRequest {
        HexLaunchRequest {
            request_id: self.next_id(),
            context,
            local_player,
            spectator,
            networked,
            spec,
            rules: perspective.0,
            seat: perspective.1,
        }
    }

    pub(super) fn reissue(&mut self, request: HexLaunchRequest) -> HexLaunchRequest {
        HexLaunchRequest {
            request_id: self.next_id(),
            ..request
        }
    }

    fn next_id(&mut self) -> HexLaunchRequestId {
        self.last_issued = self.last_issued.wrapping_add(1).max(1);
        HexLaunchRequestId(self.last_issued)
    }
}

/// Complete immutable input and presentation metadata for one launch attempt.
///
/// Simulation preparation reads only [`Self::spec`]. The remaining fields let the
/// runtime and loading screen preserve ownership, spectator, network, and back-route
/// behavior without reconstructing those decisions from UI entities.
#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HexLaunchRequest {
    pub(crate) request_id: HexLaunchRequestId,
    pub(crate) context: LaunchContext,
    pub(crate) local_player: PlayerId,
    pub(crate) spectator: bool,
    pub(crate) networked: bool,
    pub(crate) spec: HexLaunchSpec,
    pub(crate) rules: PlayRules,
    pub(crate) seat: PlaySeat,
}
