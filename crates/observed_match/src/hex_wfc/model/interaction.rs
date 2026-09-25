//! Read-only interaction affordances derived from the same state and ranges as actions.

use observed_authoring::RoomSocketKind;
use observed_core::PlayerId;

use super::{HexWfcMatch, objectives::INTERACTION_RADIUS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexInteractionAction {
    Interact,
    HoldInteract,
    DeployLantern,
    RecoverLantern,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexInteraction {
    pub action: HexInteractionAction,
    pub title: &'static str,
    pub detail: &'static str,
}

impl HexWfcMatch {
    /// Only show executable, nearby actions. Presentation must not turn every
    /// labeled room prop into a button when no command implements that prop.
    pub fn interaction(&self, id: PlayerId) -> Option<HexInteraction> {
        let player = self.players.get(&id)?;
        if !player.in_facility() {
            return None;
        }
        let objective = &self.teams.get(&player.team)?.objectives;
        if self.objectives.enabled {
            let nearest = self
                .geometry
                .sockets
                .iter()
                .filter(|socket| {
                    socket.cell == player.cell
                        && player.position.distance(socket.position) <= INTERACTION_RADIUS
                })
                .filter_map(|socket| {
                    let (title, detail, action) = match socket.kind {
                        RoomSocketKind::Keystone
                            if objective.keystones < self.objectives.keystones_required
                                && self
                                    .objectives
                                    .available_keystones
                                    .contains(&socket.room_generation_key) =>
                        {
                            (
                                "Collect keystone",
                                "Shared with your team",
                                HexInteractionAction::Interact,
                            )
                        }
                        RoomSocketKind::Monitor
                            if !self
                                .objectives
                                .surveyed_monitors
                                .contains(&(player.team, socket.room_generation_key)) =>
                        {
                            (
                                "Survey nearby rooms",
                                "Updates your team's map",
                                HexInteractionAction::Interact,
                            )
                        }
                        RoomSocketKind::StationA | RoomSocketKind::StationB
                            if !objective.dual_station_complete =>
                        {
                            (
                                "Synchronize station",
                                if self.teams[&player.team]
                                    .members
                                    .iter()
                                    .filter(|id| self.players[id].in_facility())
                                    .count()
                                    <= 1
                                {
                                    "Release to cancel"
                                } else {
                                    "Teammate holds the other console"
                                },
                                HexInteractionAction::HoldInteract,
                            )
                        }
                        _ => return None,
                    };
                    Some((
                        player.position.distance_squared(socket.position),
                        HexInteraction {
                            action,
                            title,
                            detail,
                        },
                    ))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, interaction)) = nearest {
                return Some(interaction);
            }
        }
        if self
            .lanterns
            .caches
            .values()
            .any(|cache| cache.cell == player.cell && !cache.collected)
        {
            return Some(HexInteraction {
                action: HexInteractionAction::Interact,
                title: "Collect anchor lanterns",
                detail: "Hold a connection while you explore",
            });
        }
        if self
            .lanterns
            .deployed
            .values()
            .any(|lantern| lantern.owner == id && lantern.position.distance(player.position) <= 1.8)
        {
            return Some(HexInteraction {
                action: HexInteractionAction::RecoverLantern,
                title: "Recover anchor lantern",
                detail: "This connection will be free to change",
            });
        }
        if self.lanterns.inventory(id) > 0
            && let Some(site) = self.deployable_threshold(id)
            && !self
                .lanterns
                .deployed
                .values()
                .any(|lantern| lantern.threshold == site.threshold)
        {
            return Some(HexInteraction {
                action: HexInteractionAction::DeployLantern,
                title: "Anchor this connection",
                detail: "Keeps this threshold stable when you look away",
            });
        }
        None
    }
}
