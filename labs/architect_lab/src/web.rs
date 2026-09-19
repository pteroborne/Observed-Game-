//! Browser adapter: JSON snapshots and the same authoritative card command.
//! DOM/SVG owns layout and gestures; no browser input enters the simulation directly.
use observed_facility::hex_wfc::HexSpace;
use observed_hex::{HexCoord, HexFace};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::sim::{ArchitectLab, ArchitectMode, CardKind, DoorState, ObserverState, TileShape};

#[wasm_bindgen]
pub struct RogueGame {
    pub(crate) sim: ArchitectLab,
    paused: bool,
}

#[wasm_bindgen]
impl RogueGame {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: u8) -> Result<RogueGame, String> {
        let sim = ArchitectLab::for_mode(mode_for(mode)).map_err(|error| format!("{error:?}"))?;
        Ok(Self { sim, paused: true })
    }

    pub fn reset(&mut self, mode: u8) -> Result<(), String> {
        let policy = self.sim.power_policy;
        *self = Self::new(mode)?;
        self.sim.power_policy = policy;
        Ok(())
    }

    pub fn power_policy(&self) -> String {
        self.sim.power_policy.label().to_string()
    }

    pub fn set_power_policy(&mut self, policy: &str) -> Result<(), String> {
        self.sim.power_policy = match policy.to_ascii_lowercase().as_str() {
            "oneway" | "one_way" | "one-way" => crate::sim::PowerPolicy::OneWay,
            "restorable" => crate::sim::PowerPolicy::Restorable,
            "alwayson" | "always_on" | "always-on" => crate::sim::PowerPolicy::AlwaysOn,
            _ => return Err(format!("unknown power policy: {policy}")),
        };
        Ok(())
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn set_demo(&mut self, enabled: bool) {
        self.sim.bot_architect = enabled;
    }

    pub fn advance(&mut self, ticks: u32) {
        if !self.paused {
            for _ in 0..ticks.min(60) {
                self.sim.tick();
            }
        }
    }

    pub fn play(
        &mut self,
        card: usize,
        q: u16,
        r: u16,
        level: u8,
        rotation: u8,
    ) -> Result<(), String> {
        if self.sim.bot_architect {
            return Err("Pause the demo to take control.".into());
        }
        let command = self
            .sim
            .selected_command(card, HexCoord { q, r, level }, rotation)
            .ok_or("Choose a card first.")?;
        self.sim
            .submit(command)
            .map_err(|refusal| refusal.label().to_string())
    }

    /// Legal targets and exact immediate topology consequences, even during recharge.
    /// This clone is read-only forecasting; actual submission still checks the cooldown.
    pub fn preview(&self, card: usize, q: u16, r: u16, level: u8, rotation: u8) -> String {
        let target = HexCoord { q, r, level };
        let mut before = self.sim.clone();
        before.cooldown = 0;
        let legal: Vec<_> = before
            .mutable_targets()
            .into_iter()
            .filter_map(|cell| {
                let rotations: Vec<_> = (0..6)
                    .filter(|&rot| {
                        before
                            .selected_command(card, cell, rot)
                            .is_some_and(|command| before.refusal(command).is_none())
                    })
                    .collect();
                (!rotations.is_empty())
                    .then(|| json!({"cell": coord(cell), "rotations": rotations}))
            })
            .collect();
        let Some(command) = before.selected_command(card, target, rotation) else {
            return json!({"legal": legal, "reason": "Choose a card.", "ok": false}).to_string();
        };
        if let Some(refusal) = before.refusal(command) {
            return json!({"legal": legal, "reason": refusal.label(), "ok": false}).to_string();
        }
        let before_routes = pursuit_routes(&before);
        let mut after = before.clone();
        after.submit(command).expect("preview legality checked");
        let after_routes = pursuit_routes(&after);
        let before_steps = before_routes.iter().map(Vec::len).min();
        let after_steps = after_routes.iter().map(Vec::len).min();
        let gained = after
            .guardian_reachable()
            .len()
            .saturating_sub(before.guardian_reachable().len());
        let route_effect = match (before_steps, after_steps) {
            (None, Some(_)) => "Opens a pursuit route",
            (Some(_), None) => "Cuts the current pursuit route",
            (Some(a), Some(b)) if b < a => "Shortens the pursuit route",
            (Some(a), Some(b)) if b > a => "Lengthens the pursuit route",
            _ if gained > 0 => "Opens more ground for the Guardian",
            _ => "Changes the connections",
        };
        json!({"legal": legal, "ok": true, "reason": route_effect,
            "reachable_gain": gained,
            "before_steps": before_steps.map(|v| v.saturating_sub(1)),
            "after_steps": after_steps.map(|v| v.saturating_sub(1)),
            "route": after_routes.first().cloned().or_else(|| {
                after.guardians.values().find_map(|guardian| after.route(guardian.cell, target))
                    .map(|route| route.into_iter().map(coord).collect::<Vec<_>>())
            }),
            "unstable": after.contradictions.iter().copied().map(coord).collect::<Vec<_>>(),
            "repaired": before.contradictions.len().saturating_sub(after.contradictions.len()),
            "doors": after.world.placements.get(&target).map(|tile| tile.doors),
        })
        .to_string()
    }

    /// Browser rendering consumes the same face order and rotations as commands.
    pub fn render_contract(&self) -> String {
        let faces: Vec<_> = HexFace::LATERAL.into_iter().map(|face| {
            let (q, r, _) = face.delta();
            json!({"index": face.index(), "name": format!("{face:?}"), "delta": [q, r], "opposite": face.opposite().index()})
        }).collect();
        let shapes: Vec<_> = TileShape::ALL
            .into_iter()
            .map(|shape| {
                json!({
                    "name": shape.label().split(" / ").next().unwrap_or("tile"),
                    "rotations": (0..6).map(|rotation| shape.doors(rotation)).collect::<Vec<_>>()
                })
            })
            .collect();
        json!({"faces": faces, "shapes": shapes}).to_string()
    }

    pub fn snapshot(&self) -> String {
        let sim = &self.sim;
        let detected = sim.detected_observers();
        let next = sim.next_retraction();
        let cells: Vec<_> = sim.known.iter().filter_map(|&cell| {
            let tile = sim.world.placements.get(&cell)?;
            Some(json!({"cell": coord(cell), "solid": tile.space != HexSpace::Void,
                "doors": tile.doors, "observed": sim.observed.contains(&cell),
                "prison": sim.prison_core.contains(&cell),
                "unstable": sim.contradictions.contains(&cell),
                "condemned": sim.condemned.is_some_and(|(c, _)| c == cell),
                "retracted": sim.retracted.contains(&cell),
                "next": next == Some(cell),
                "held": sim.retraction_protected(cell),
                "vertical": sim.exits(cell).into_iter().filter(|other| other.level != cell.level).map(coord).collect::<Vec<_>>()
            }))
        }).collect();
        let observers: Vec<_> = sim
            .observers
            .values()
            .filter(|observer| {
                observer.state == ObserverState::Jailed
                    || observer.state == ObserverState::Corrupted
                    || detected.contains(&observer.id)
            })
            .map(|observer| {
                json!({
                    "id": observer.id.0,
                    "team": observer.team.0,
                    "cell": coord(observer.cell),
                    "facing": observer.facing.index(),
                    "jailed": observer.state == ObserverState::Jailed,
                    "corrupted": observer.state == ObserverState::Corrupted,
                })
            })
            .collect();
        let guardians: Vec<_> = sim.guardians.values().map(|guardian| json!({"id": guardian.id.0,
            "cell": coord(guardian.cell), "held": sim.observed.contains(&guardian.cell),
            "intent": sim.traces.get(&format!("Guardian {}", guardian.id.0)).and_then(|trace| trace.selected)
        })).collect();
        let cards: Vec<_> = sim
            .deck
            .hand
            .iter()
            .map(|card| {
                let (name, doors, purpose) = match card.kind {
                    CardKind::Tile(shape) => (
                        shape.label().split(" / ").next().unwrap_or("tile"),
                        shape.base_doors(),
                        "Rewrite or extend a route",
                    ),
                    CardKind::Door => ("door", 0, "Block a threshold; Observers can reopen it"),
                };
                let rotations: Vec<_> = (0..6).map(|rotation| match card.kind {
                    CardKind::Tile(shape) => shape.doors(rotation),
                    CardKind::Door => 1 << rotation,
                }).collect();
                json!({"id": card.id.0, "name": name, "doors": doors, "rotations": rotations, "purpose": purpose,
                "district": card.district.map(|district| district.label())})
            })
            .collect();
        let doors: Vec<_> = sim.doors.iter().filter_map(|(key, state)| {
            let next = sim.world.config.grid().neighbor(key.cell, key.face)?;
            Some(json!({"from": coord(key.cell), "to": coord(next), "open": *state == DoorState::Open}))
        }).collect();
        let events: Vec<_> = sim
            .events
            .iter()
            .rev()
            .take(6)
            .map(|event| {
                json!({
                    "tick": event.tick, "kind": format!("{:?}", event.kind),
                    "cell": event.cell.map(coord), "message": event.message,
                })
            })
            .collect();
        json!({"tick": sim.tick, "paused": self.paused, "demo": sim.bot_architect,
            "mode": sim.mode.label(), "levels": sim.world.config.levels,
            "power_policy": sim.power_policy.label(),
            "cols": sim.world.config.cols, "rows": sim.world.config.rows,
            "cooldown": sim.cooldown, "outcome": format!("{:?}", sim.outcome),
            "caught": sim.observers.values().filter(|observer| observer.state == ObserverState::Jailed).count(),
            "corrupted": sim.observers.values().filter(|observer| observer.state == ObserverState::Corrupted).count(),
            "total": sim.observers.len(), "detected": detected.len(), "plays": sim.command_log.len(),
            "collapse_in": sim.next_retraction_tick.map(|tick| tick.saturating_sub(sim.tick)),
            "cells": cells, "observers": observers, "guardians": guardians, "cards": cards,
            "doors": doors, "events": events, "routes": pursuit_routes(sim),
            "collapsed_floors": sim.collapsed_floors,
        }).to_string()
    }

    pub fn theme(&self) -> String {
        use observed_style::{SchematicRole, TacticsRole};
        let colors = [
            ("screen", observed_style::schematic_screen()),
            (
                "selected",
                observed_style::schematic(SchematicRole::Selected).base_color,
            ),
            (
                "safe",
                observed_style::schematic(SchematicRole::Pinned).base_color,
            ),
            (
                "danger",
                observed_style::schematic(SchematicRole::Volatile).base_color,
            ),
            (
                "watched",
                observed_style::tactics(TacticsRole::ReachableRoute).base_color,
            ),
            (
                "structure",
                observed_style::architecture_tactical(
                    observed_content::ArchitectureRegister::Institutional,
                )
                .base_color,
            ),
            (
                "guardian",
                observed_style::marker(observed_style::MarkerRole::Director).base_color,
            ),
        ];
        let palette: serde_json::Map<String, Value> = colors
            .into_iter()
            .map(|(name, color)| {
                let c = color.to_srgba();
                (
                    name.into(),
                    json!(format!(
                        "rgb({:.0} {:.0} {:.0})",
                        c.red * 255.,
                        c.green * 255.,
                        c.blue * 255.
                    )),
                )
            })
            .collect();
        Value::Object(palette).to_string()
    }
}

fn mode_for(mode: u8) -> ArchitectMode {
    ArchitectMode::ALL[usize::from(mode).min(2)]
}
fn coord(cell: HexCoord) -> [u16; 3] {
    [cell.q, cell.r, u16::from(cell.level)]
}
fn pursuit_routes(sim: &ArchitectLab) -> Vec<Vec<[u16; 3]>> {
    let detected = sim.detected_observers();
    let mut routes: Vec<_> = sim
        .guardians
        .values()
        .flat_map(|guardian| {
            sim.observers
                .values()
                .filter(|observer| detected.contains(&observer.id))
                .filter_map(move |observer| sim.route(guardian.cell, observer.cell))
        })
        .map(|route| route.into_iter().map(coord).collect::<Vec<_>>())
        .collect();
    routes.sort_by_key(Vec::len);
    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_and_rejected_browser_commands_do_not_change_authority() {
        let mut game = RogueGame::new(0).unwrap();
        let command = game.sim.legal_commands()[0];
        let crate::sim::ArchitectCommand::Play {
            target, rotation, ..
        } = command
        else {
            panic!("expected play command");
        };
        let before = game.snapshot();
        let _ = game.preview(0, target.q, target.r, target.level, rotation);
        assert_eq!(game.snapshot(), before);
        game.sim.submit(command).unwrap();
        let before = game.snapshot();
        assert!(
            game.play(0, target.q, target.r, target.level, rotation)
                .is_err()
        );
        assert_eq!(game.snapshot(), before);
    }

    #[test]
    fn browser_starts_paused_and_reset_restores_the_complete_session() {
        let mut game = RogueGame::new(0).unwrap();
        let initial = game.snapshot();
        game.advance(60);
        assert_eq!(game.snapshot(), initial);
        game.set_paused(false);
        game.advance(600);
        assert_eq!(
            game.sim.tick, 60,
            "background gaps cannot produce an unbounded catch-up"
        );
        game.set_demo(true);
        game.reset(0).unwrap();
        assert_eq!(game.snapshot(), initial);
    }

    #[test]
    fn browser_power_policy_survives_reset() {
        let mut game = RogueGame::new(0).unwrap();
        assert_eq!(game.power_policy(), "Restorable");
        game.set_power_policy("one_way").unwrap();
        assert_eq!(game.power_policy(), "One-Way");
        game.reset(0).unwrap();
        assert_eq!(game.power_policy(), "One-Way");
    }

    #[test]
    fn browser_snapshot_omits_undetected_prey() {
        let game = RogueGame::new(0).unwrap();
        let snapshot: Value = serde_json::from_str(&game.snapshot()).unwrap();
        assert_eq!(
            snapshot["observers"].as_array().unwrap().len(),
            game.sim.detected_observers().len()
        );
        assert!(snapshot["observers"].as_array().unwrap().is_empty());
    }
}
