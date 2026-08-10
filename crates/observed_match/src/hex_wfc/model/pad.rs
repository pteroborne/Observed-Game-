//! Team-linked teleport pads for the hex match.
//!
//! A pad is a floor plate. Two of them, dropped by the same team, form one
//! two-way link: step onto either and you are standing on the other. That is
//! the whole mechanic, and it is deliberately the whole mechanic — the pads
//! carry no charge, no cooldown beyond the re-arm below, and no direction.
//!
//! # Ported from the demoted model, trigger and all
//!
//! `elimination::TeamToolState` defined two pads per team and a
//! `pad_link_target` that refused to complete across teams, but it never said
//! what *fires* the link, because that model had no bodies to stand anywhere.
//! Stepping on the plate is the trigger here, chosen so the tool needs no HUD
//! to explain it: the plate is a visible object, walking onto it is the verb,
//! and nothing has to be read off a screen edge.
//!
//! # The re-arm is what makes stepping safe
//!
//! Auto-firing on contact has one failure mode and it is fatal without a guard:
//! arriving at the far pad means *standing on a pad*, which fires the link back,
//! forever. So a player who has just been moved — or who has just placed a pad
//! under their own feet — is suppressed for [`PAD_REARM_TICKS`] before any pad
//! will move them again. Suppression is per player, counted in simulation ticks,
//! and lives in the snapshot, because a peer that disagreed about it would
//! diverge on the very next contact.
//!
//! # Keyed by team, attributed to a player
//!
//! The original test is named `teleport_pads_are_keyed_to_their_team`, and that
//! is the behaviour kept: a rival's plate never completes your link, but a
//! teammate's does, so a pair placed by two people still works. The deploying
//! player is recorded anyway — presentation colours a plate by its owner, and a
//! future per-player roster can tighten the rule without moving stable IDs.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::{EquipmentId, PlayerId, TeamId};
use observed_hex::HexCoord;

use super::{HexActionButtons, HexMatchEvent, HexMatchEventKind, HexWfcMatch};

/// Pads each player carries at match start. Two, because one pad links nothing.
pub const PADS_PER_PLAYER: u16 = 2;

/// Ticks a player is immune to pad contact after being moved by one, or after
/// placing one under their own feet. At 60 Hz this is one second — long enough
/// to walk clear of a plate, short enough that a link stays usable under chase.
pub const PAD_REARM_TICKS: u16 = 60;

/// How close a body's centre must be, horizontally, to count as standing on a
/// plate. Slightly under a cell's inradius so two pads in neighbouring cells can
/// never both claim the same body.
pub const PAD_CONTACT_RADIUS: f32 = 0.9;

#[derive(Clone, Debug, PartialEq)]
pub struct HexDeployedPad {
    pub id: EquipmentId,
    /// The player who placed it. Attribution, not authorisation — see the module
    /// note; the link is completed by [`HexDeployedPad::team`].
    pub owner: PlayerId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub position: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexPadState {
    pub carried: BTreeMap<PlayerId, u16>,
    pub deployed: BTreeMap<EquipmentId, HexDeployedPad>,
    /// Ticks remaining before this player may be moved by a pad again. Absent
    /// means ready. Entries are dropped at zero so the map stays small and its
    /// snapshot ordering stays stable.
    pub suppressed: BTreeMap<PlayerId, u16>,
    next_id: u32,
}

impl HexPadState {
    #[must_use]
    pub fn new(players: impl IntoIterator<Item = PlayerId>) -> Self {
        Self {
            carried: players
                .into_iter()
                .map(|player| (player, PADS_PER_PLAYER))
                .collect(),
            deployed: BTreeMap::new(),
            suppressed: BTreeMap::new(),
            next_id: 0,
        }
    }

    #[must_use]
    pub fn inventory(&self, player: PlayerId) -> u16 {
        self.carried.get(&player).copied().unwrap_or(0)
    }

    /// Place a plate at `position`. `None` when the player is carrying none.
    pub fn deploy(
        &mut self,
        owner: PlayerId,
        team: TeamId,
        cell: HexCoord,
        position: Vec3,
    ) -> Option<EquipmentId> {
        let carried = self.carried.get_mut(&owner)?;
        if *carried == 0 {
            return None;
        }
        *carried -= 1;
        let id = self.allocate_id();
        self.deployed.insert(
            id,
            HexDeployedPad {
                id,
                owner,
                team,
                cell,
                position,
            },
        );
        // The plate lands under the placer's feet, which is contact. Without this
        // the second pad of a pair would fire the instant it was set down.
        self.suppress(owner);
        Some(id)
    }

    /// The plate this body is standing on, if any belongs to `team`.
    #[must_use]
    pub fn pad_under(
        &self,
        team: TeamId,
        cell: HexCoord,
        position: Vec3,
    ) -> Option<&HexDeployedPad> {
        self.deployed.values().find(|pad| {
            pad.team == team
                && pad.cell == cell
                && horizontal_distance_squared(pad.position, position)
                    <= PAD_CONTACT_RADIUS * PAD_CONTACT_RADIUS
        })
    }

    /// Where `from` sends a member of `team`: the team's other plate. `None`
    /// while the pair is incomplete, which is the whole of "one pad links
    /// nothing". With more than two down the lowest remaining ID wins, so the
    /// choice never depends on map iteration order changing under us.
    #[must_use]
    pub fn link_target(&self, team: TeamId, from: EquipmentId) -> Option<&HexDeployedPad> {
        self.deployed
            .values()
            .find(|pad| pad.team == team && pad.id != from)
    }

    #[must_use]
    pub fn is_suppressed(&self, player: PlayerId) -> bool {
        self.suppressed.contains_key(&player)
    }

    pub fn suppress(&mut self, player: PlayerId) {
        self.suppressed.insert(player, PAD_REARM_TICKS);
    }

    /// One tick of the re-arm clock. Entries reaching zero are removed rather
    /// than left at zero, so `suppressed` and its digest contribution describe
    /// exactly the players who are currently immune.
    pub fn tick_suppression(&mut self) {
        self.suppressed.retain(|_, remaining| {
            *remaining = remaining.saturating_sub(1);
            *remaining > 0
        });
    }

    fn allocate_id(&mut self) -> EquipmentId {
        let id = EquipmentId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

fn horizontal_distance_squared(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

impl HexWfcMatch {
    /// Placement. Runs with the rest of a player's action buttons.
    pub(super) fn step_pad_actions(&mut self, player: PlayerId, actions: HexActionButtons) {
        if !actions.deploy_pad || self.players[&player].escaped {
            return;
        }
        let (cell, position, team) = {
            let state = &self.players[&player];
            (state.cell, state.position, state.team)
        };
        // A plate lies on the deck, and `HexPlayerState::position` is the centre
        // of the body box — `FpsBody`: "Feet are at `position.y - half_height`".
        // Dropping the raw position hung every plate at chest height. The offset
        // comes from the traversal profile rather than a literal, because that
        // is where the capsule's own dimensions are declared and a second copy
        // would drift from the body it is supposed to describe.
        let feet = position
            - Vec3::Y
                * self
                    .content
                    .traversal_profile()
                    .requirements()
                    .capsule_half_height;
        if self.pads.deploy(player, team, cell, feet).is_some() {
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::PadDeployed,
                player: Some(player),
                cell: Some(cell),
            });
        }
    }

    /// Contact. Runs after every body has moved this tick, so a player is judged
    /// against where they ended up rather than where they started; the existing
    /// `sync_teleports_to_bodies` later in the step carries the jump into the
    /// physics body exactly as a spawn does.
    pub(super) fn step_pad_contacts(&mut self) {
        self.pads.tick_suppression();
        // Plates are stored on the deck; a body is positioned by its centre. The
        // two frames differ by exactly this, and conflating them either buries a
        // traveller to the waist or floats them.
        let half_height = self
            .content
            .traversal_profile()
            .requirements()
            .capsule_half_height;
        let candidates = self
            .players
            .values()
            .filter(|player| !player.escaped)
            .map(|player| (player.id, player.team, player.cell, player.position))
            .collect::<Vec<_>>();
        for (id, team, cell, position) in candidates {
            if self.pads.is_suppressed(id) {
                continue;
            }
            let Some(from) = self.pads.pad_under(team, cell, position).map(|pad| pad.id) else {
                continue;
            };
            let Some(target) = self
                .pads
                .link_target(team, from)
                .map(|pad| (pad.cell, pad.position))
            else {
                continue;
            };
            let player = self.players.get_mut(&id).expect("stepped player");
            player.cell = target.0;
            player.position = target.1 + Vec3::Y * half_height;
            self.pads.suppress(id);
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::PadTraversed,
                player: Some(id),
                cell: Some(target.0),
            });
        }
    }
}
