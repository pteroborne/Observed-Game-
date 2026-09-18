//! Deterministic role selectors. Trees choose intents; application remains in
//! the authoritative simulation.

use observed_hex::{HexCoord, travel_distance};

use crate::economy::{MAX_CHARGE, SHOVE_COST};

use super::{
    ArchitectCommand, ArchitectLab, BehaviorTrace, DoorState, GuardianId, GuardianKind, ObserverId,
    ObserverState, ThresholdKey, command_key, face_between, threshold_touches,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObserverIntent {
    Hold,
    HoldGuardian,
    Step(HexCoord),
    SetDoor(ThresholdKey, DoorState),
    Shove(GuardianId),
    ToggleGenerator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardianIntent {
    Hold,
    Step(HexCoord, Option<HexCoord>),
    Capture(ObserverId),
}

impl ArchitectLab {
    pub(crate) fn observer_intent(&self, id: ObserverId) -> (ObserverIntent, BehaviorTrace) {
        let observer = &self.observers[&id];
        let mut trace = BehaviorTrace::default();
        if trace.test("held in prison", observer.state == ObserverState::Jailed) {
            return (ObserverIntent::Hold, trace);
        }

        // Shove adjacent Minor Guardian if charged
        let shove_target = if self.economy.charge(id) >= SHOVE_COST {
            self.guardians.values().find(|guardian| {
                guardian.kind == GuardianKind::Minor
                    && travel_distance(observer.cell, guardian.cell) == 1
            })
        } else {
            None
        };
        if trace.test("shove adjacent Minor Guardian", shove_target.is_some())
            && let Some(guardian) = shove_target
        {
            return (ObserverIntent::Shove(guardian.id), trace);
        }

        let visible_guardian = self.guardians.values().find(|guardian| {
            guardian.kind == GuardianKind::Major
                && (guardian.cell == observer.cell
                    || self.step_through(observer.cell, observer.facing) == Some(guardian.cell))
        });
        if trace.test(
            "hold visible Guardian",
            visible_guardian.is_some() && observer.hold_beats < 2,
        ) {
            return (ObserverIntent::HoldGuardian, trace);
        }
        if let Some(guardian) = visible_guardian
            && let Some(key) = self.threshold_between(observer.cell, guardian.cell)
            && self.doors.get(&key) == Some(&DoorState::Open)
            && self.economy.is_powered(key.cell.level)
        {
            trace.visited.push("close door on Guardian");
            trace.selected = Some("close door on Guardian");
            return (ObserverIntent::SetDoor(key, DoorState::Closed), trace);
        }

        // Restore floor power if standing at an unpowered generator
        if trace.test(
            "restore floor power at generator",
            !self.economy.is_powered(observer.cell.level)
                && self.economy.is_at_generator(observer.cell),
        ) {
            return (ObserverIntent::ToggleGenerator, trace);
        }

        let danger = self
            .guardians
            .values()
            .any(|guardian| travel_distance(observer.cell, guardian.cell) <= 2);
        if trace.test("evade immediate danger", danger)
            && let Some(next) = self.safest_exit(observer.cell)
        {
            return (ObserverIntent::Step(next), trace);
        }

        // Seek generator if floor is unpowered and not in immediate danger
        let gen_step = if !self.economy.is_powered(observer.cell.level) {
            self.economy
                .generators
                .get(&observer.cell.level)
                .copied()
                .and_then(|generator_cell| self.route(observer.cell, generator_cell))
                .and_then(|path| path.get(1).copied())
        } else {
            None
        };
        if trace.test("seek generator to restore power", gen_step.is_some())
            && let Some(next) = gen_step
        {
            return (ObserverIntent::Step(next), trace);
        }

        // Recharge at powered station if charge is below maximum
        if trace.test(
            "recharge at station",
            self.economy.charge(id) < MAX_CHARGE
                && self.economy.is_at_powered_station(observer.cell),
        ) {
            return (ObserverIntent::Hold, trace);
        }

        // Seek nearest powered recharge station if charge is depleted (< SHOVE_COST)
        let station_step = if self.economy.charge(id) < SHOVE_COST {
            self.economy
                .stations
                .iter()
                .filter(|&st| st.level == observer.cell.level && self.economy.is_powered(st.level))
                .filter_map(|&st| self.route(observer.cell, st).map(|path| (path.len(), path)))
                .min_by_key(|(len, _)| *len)
                .and_then(|(_, path)| path.get(1).copied())
        } else {
            None
        };
        if trace.test("seek recharge station", station_step.is_some())
            && let Some(next) = station_step
        {
            return (ObserverIntent::Step(next), trace);
        }

        let summit = self.world.config.exit();
        if trace.test("advance summit", observer.cell != summit) {
            if let Some(next) = self
                .route(observer.cell, summit)
                .and_then(|path| path.get(1).copied())
            {
                return (ObserverIntent::Step(next), trace);
            }
            if let Some(key) = self.closed_door_restoring_route(observer.cell, summit)
                && self.economy.is_powered(key.cell.level)
            {
                trace.visited.push("open door toward summit");
                trace.selected = Some("open door toward summit");
                return (ObserverIntent::SetDoor(key, DoorState::Open), trace);
            }
        }
        trace.test("watch another approach", true);
        (ObserverIntent::Hold, trace)
    }

    fn safest_exit(&self, from: HexCoord) -> Option<HexCoord> {
        self.exits(from).into_iter().max_by_key(|&candidate| {
            let nearest = self
                .guardians
                .values()
                .filter_map(|guardian| self.route(candidate, guardian.cell).map(|path| path.len()))
                .min()
                .unwrap_or(usize::MAX);
            (nearest, std::cmp::Reverse(candidate))
        })
    }

    pub(crate) fn apply_observer_intent(&mut self, id: ObserverId, intent: ObserverIntent) {
        match intent {
            ObserverIntent::HoldGuardian => {
                let observer = self.observers.get_mut(&id).expect("known Observer");
                observer.hold_beats += 1;
            }
            ObserverIntent::Hold => {
                let observer = self.observers.get_mut(&id).expect("known Observer");
                observer.hold_beats = 0;
                observer.facing = super::lateral_face((observer.facing.index() as u8 + 1) % 6);
            }
            ObserverIntent::Step(next) => {
                let observer = self.observers.get_mut(&id).expect("known Observer");
                if let Some(face) = face_between(self.world.config, observer.cell, next) {
                    observer.facing = face;
                }
                observer.cell = next;
                observer.hold_beats = 0;
            }
            ObserverIntent::SetDoor(key, state) => {
                let observer = self.observers.get_mut(&id).expect("known Observer");
                if self.economy.is_powered(key.cell.level) {
                    self.doors.insert(key, state);
                }
                observer.hold_beats = 0;
            }
            ObserverIntent::Shove(target_guardian) => {
                let _ = self.shove(id, target_guardian);
                if let Some(observer) = self.observers.get_mut(&id) {
                    observer.hold_beats = 0;
                }
            }
            ObserverIntent::ToggleGenerator => {
                let _ = self.toggle_generator(id);
                if let Some(observer) = self.observers.get_mut(&id) {
                    observer.hold_beats = 0;
                }
            }
        }
    }

    pub(super) fn threshold_between(&self, from: HexCoord, to: HexCoord) -> Option<ThresholdKey> {
        let face = face_between(self.world.config, from, to)?;
        self.threshold_key(from, face)
    }

    fn closed_door_restoring_route(&self, from: HexCoord, goal: HexCoord) -> Option<ThresholdKey> {
        self.doors
            .iter()
            .filter(|(_, state)| **state == DoorState::Closed)
            .filter(|(key, _)| threshold_touches(**key, from, &self.world))
            .map(|(&key, _)| key)
            .find(|&key| {
                let mut preview = self.clone();
                preview.doors.insert(key, DoorState::Open);
                preview.route(from, goal).is_some()
            })
    }

    pub(crate) fn guardian_intent(&self, id: GuardianId) -> (GuardianIntent, BehaviorTrace) {
        let guardian = &self.guardians[&id];
        let mut trace = BehaviorTrace::default();
        if guardian.kind == GuardianKind::Major
            && trace.test(
                "frozen while observed",
                self.observed.contains(&guardian.cell),
            )
        {
            return (GuardianIntent::Hold, trace);
        }
        if guardian.kind == GuardianKind::Minor {
            trace.test("frozen while observed", false);
        }
        if let Some(observer) = self.observers.values().find(|observer| {
            observer.state == ObserverState::Active && observer.cell == guardian.cell
        }) {
            trace.test("capture adjacent", true);
            return (GuardianIntent::Capture(observer.id), trace);
        }
        trace.test("capture adjacent", false);
        let detected = self.detected_observers();
        let target = self
            .observers
            .values()
            .filter(|observer| {
                observer.state == ObserverState::Active && detected.contains(&observer.id)
            })
            .filter_map(|observer| {
                let path = if guardian.kind == GuardianKind::Minor {
                    self.route_minor(guardian.cell, observer.cell)?
                } else {
                    self.route(guardian.cell, observer.cell)?
                };
                Some((path.len(), observer.id, observer.cell, path))
            })
            .min_by_key(|(len, id, _, _)| (*len, *id));
        if trace.test("pursue detected", target.is_some()) {
            let (_, _, target_cell, path) = target.expect("branch proved target");
            if path.len() <= 1 {
                let observer = self
                    .observers
                    .values()
                    .find(|observer| {
                        observer.state == ObserverState::Active && observer.cell == target_cell
                    })
                    .expect("target is active");
                return (GuardianIntent::Capture(observer.id), trace);
            }
            return (GuardianIntent::Step(path[1], Some(target_cell)), trace);
        }
        if guardian.kind == GuardianKind::Major
            && trace.test("obey Rogue directive", self.rogue_directive.is_some())
            && let Some(next) = self
                .rogue_directive
                .and_then(|target| self.route(guardian.cell, target))
                .and_then(|path| path.get(1).copied())
        {
            return (GuardianIntent::Step(next, None), trace);
        }
        if guardian.kind == GuardianKind::Minor {
            trace.test("obey Rogue directive", false);
        }
        if trace.test(
            "investigate last detection",
            guardian.last_detection.is_some(),
        ) && let Some(next) = guardian
            .last_detection
            .and_then(|target| {
                if guardian.kind == GuardianKind::Minor {
                    self.route_minor(guardian.cell, target)
                } else {
                    self.route(guardian.cell, target)
                }
            })
            .and_then(|path| path.get(1).copied())
        {
            return (GuardianIntent::Step(next, None), trace);
        }
        trace.test("patrol", true);
        let exits = if guardian.kind == GuardianKind::Minor {
            self.exits_minor(guardian.cell)
        } else {
            self.exits(guardian.cell)
        };
        (
            exits
                .into_iter()
                .min_by_key(|cell| {
                    (
                        self.guardian_visits.get(&(id, *cell)).copied().unwrap_or(0),
                        *cell,
                    )
                })
                .map_or(GuardianIntent::Hold, |next| {
                    GuardianIntent::Step(next, None)
                }),
            trace,
        )
    }

    pub(super) fn apply_guardian_intent(&mut self, id: GuardianId, intent: GuardianIntent) {
        match intent {
            GuardianIntent::Hold => {}
            GuardianIntent::Step(next, detection) => {
                let guardian = self.guardians.get_mut(&id).expect("known Guardian");
                if guardian.kind == GuardianKind::Minor && self.prison_core.contains(&next) {
                    return;
                }
                guardian.cell = next;
                *self.guardian_visits.entry((id, next)).or_default() += 1;
                if self.rogue_directive == Some(next) {
                    self.rogue_directive = None;
                }
                if guardian.last_detection == Some(next) {
                    guardian.last_detection = None;
                }
                if detection.is_some() {
                    guardian.last_detection = detection;
                }
                self.capture_on_cell(id);
            }
            GuardianIntent::Capture(observer) => self.jail(observer),
        }
    }

    fn capture_on_cell(&mut self, guardian: GuardianId) {
        let cell = self.guardians[&guardian].cell;
        if let Some(observer) = self
            .observers
            .values()
            .find(|observer| observer.state == ObserverState::Active && observer.cell == cell)
            .map(|observer| observer.id)
        {
            self.jail(observer);
        }
    }

    pub(super) fn jail(&mut self, observer: ObserverId) {
        let prison = self
            .prison_core
            .iter()
            .find(|cell| cell.level == 0)
            .copied()
            .expect("floor zero prison core exists");
        let observer = self.observers.get_mut(&observer).expect("known Observer");
        observer.cell = prison;
        observer.state = ObserverState::Jailed;
        self.record_event(
            super::LabEventKind::Captured,
            Some(prison),
            "Observer captured. Sent to the protected prison core.",
        );
    }

    pub(crate) fn architect_intent(&self) -> (Option<ArchitectCommand>, BehaviorTrace) {
        let mut trace = BehaviorTrace::default();
        if trace.test("wait for cooldown", self.cooldown > 0) {
            return (None, trace);
        }
        let commands = self.legal_commands();
        if commands.is_empty() {
            trace.test("hold card", true);
            return (None, trace);
        }
        let baseline = self.guardian_route_score();
        let mut scored: Vec<_> = commands
            .iter()
            .copied()
            .map(|command| {
                let mut preview = self.clone();
                preview.bot_architect = false;
                preview
                    .submit(command)
                    .expect("enumerated command is legal");
                (
                    preview.guardian_route_score(),
                    preview.contradictions.len(),
                    command,
                )
            })
            .collect();
        scored.sort_by_key(|(score, _, command)| (*score, command_key(*command)));
        let best = scored.first().copied();
        if trace.test(
            "shorten Guardian route",
            best.is_some_and(|(score, _, _)| score < baseline),
        ) {
            return (best.map(|(_, _, command)| command), trace);
        }

        // Release disturbance wave if a legal command crosses threshold
        let wave_play = commands
            .iter()
            .copied()
            .filter(|&command| {
                let mut preview = self.clone();
                preview.bot_architect = true;
                if preview.submit(command).is_ok() {
                    let ArchitectCommand::Play { target, .. } = command;
                    preview.economy.wave_count(target.level) > self.economy.wave_count(target.level)
                } else {
                    false
                }
            })
            .min_by_key(|&command| command_key(command));
        if trace.test("release disturbance wave", wave_play.is_some()) {
            return (wave_play, trace);
        }

        // Generator play: contest generator access by playing a door or contradiction at/near generator
        let generator_play = commands
            .iter()
            .copied()
            .filter(|&command| {
                let ArchitectCommand::Play { target, card, .. } = command;
                let Some(generator_cell) = self.economy.generators.get(&target.level).copied()
                else {
                    return false;
                };
                if travel_distance(target, generator_cell) > 1 {
                    return false;
                }
                let is_door = self
                    .deck
                    .hand
                    .iter()
                    .any(|c| c.id == card && c.kind == super::CardKind::Door);
                if is_door {
                    return true;
                }
                let mut preview = self.clone();
                preview.bot_architect = true;
                preview.submit(command).is_ok() && preview.contradictions.contains(&target)
            })
            .min_by_key(|&command| command_key(command));
        if trace.test("contest generator", generator_play.is_some()) {
            return (generator_play, trace);
        }

        // Explore facility truth without consulting undetected Observer positions.
        let baseline_reach = self.guardian_reachable().len();
        let progress = scored
            .iter()
            .filter_map(|(_, contradictions, command)| {
                let mut preview = self.clone();
                preview.submit(*command).ok()?;
                let reach = preview.guardian_reachable().len();
                (reach > baseline_reach).then_some((
                    *contradictions,
                    std::cmp::Reverse(reach),
                    command_key(*command),
                    *command,
                ))
            })
            .min_by_key(|(contradictions, reach, key, _)| (*contradictions, *reach, *key));
        if trace.test("extend Guardian route", progress.is_some()) {
            return (progress.map(|(_, _, _, command)| command), trace);
        }
        trace.test("hold card", true);
        (None, trace)
    }

    #[must_use]
    pub fn legal_commands(&self) -> Vec<ArchitectCommand> {
        let mut out = Vec::new();
        for card in &self.deck.hand {
            for target in self.mutable_targets() {
                for rotation in 0..6 {
                    let command = ArchitectCommand::Play {
                        card: card.id,
                        target,
                        rotation,
                    };
                    if self.refusal(command).is_none() {
                        out.push(command);
                    }
                }
            }
        }
        out.sort_by_key(|command| command_key(*command));
        out
    }

    pub fn guardian_route_score(&self) -> usize {
        let detected = self.detected_observers();
        self.guardians
            .values()
            .flat_map(|guardian| {
                self.observers
                    .values()
                    .filter(|observer| {
                        observer.state == ObserverState::Active && detected.contains(&observer.id)
                    })
                    .map(move |observer| {
                        self.route(guardian.cell, observer.cell)
                            .map_or(usize::MAX / 4, |path| path.len())
                    })
            })
            .min()
            .unwrap_or(0)
    }
}
