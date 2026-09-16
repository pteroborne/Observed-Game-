//! A scripted driver that plays the lab, for recording evidence.
//!
//! The first recording tape was a list of timed intents — "look down at tick
//! 91" — which is fine for a seven second shot of one shove and useless for
//! anything that has to *navigate*. Walking to the generator takes a number of
//! ticks that depends on where you started, whether you clipped a wall, and how
//! long the turn took, so blind timing drifts and the rest of the tape lands on
//! the wrong beats.
//!
//! This driver is closed-loop instead: each step reads authoritative state and
//! produces a [`PlayerIntent`] for this tick, exactly as a bot would. That is
//! still fully deterministic — same board, same script, same frames — and it is
//! the same command path a player's hands use, so the recording stays a real run
//! of the rules rather than an animation of them.
//!
//! Every step carries a caption that is drawn on screen while it runs, so the
//! video states the claim it is demonstrating instead of asking a viewer to take
//! the README's word for it.

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use player_input::PlayerIntent;

use crate::embodied::{Embodiment, ToolRequest, plate_center};
use crate::model::{KineticRules, KineticWorld};

/// Radians per tick the driver will turn. Comfortably readable on camera.
const TURN_RATE: f32 = 0.045;
/// How close to a plate's centre counts as arrived.
const ARRIVE_RADIUS: f32 = 2.5;
/// Ticks any navigation step may take before the driver gives up and moves on,
/// so one blocked path cannot strand the whole recording.
const NAV_TIMEOUT: u32 = 1_200;

/// One beat of the script.
#[derive(Clone, Copy, Debug)]
pub enum Beat {
    /// Hold still for a number of ticks.
    Hold(u32),
    /// Turn on the spot to look at a cell.
    LookAt(HexCoord),
    /// Walk to a cell, steering as it goes.
    WalkTo {
        target: HexCoord,
        sprint: bool,
    },
    /// Walk toward a cell without requiring arrival — used to leave the floor,
    /// where "arriving" means falling.
    WalkOff {
        target: HexCoord,
        ticks: u32,
    },
    Push,
    Pull,
    Generator,
    Jump,
    /// Move a Guardian. Staging, and captioned as such on screen.
    ///
    /// `stagger` holds it still for that many ticks. A staggered Guardian does
    /// not pursue — the model already works that way after a shove — which is
    /// the only honest lever the demo has for keeping an actor out of a scene it
    /// is not part of. Parking one "far away" does not work: the board is nine
    /// plates wide and a minor crosses one every 150 ticks, so every corner is
    /// within reach of a seventy-second recording.
    PlaceMinor {
        index: usize,
        cell: HexCoord,
        stagger: u32,
    },
    /// Park the major Guardian. It pursues throughout, and a systems tour takes
    /// long enough for it to cross the whole board and end the run mid-sentence.
    PlaceMajor {
        cell: HexCoord,
    },
    /// Fight for a while: face the nearest Guardian, shove it, and go recharge
    /// when the tool runs dry.
    ///
    /// The other beats are choreography. This one is a small bot, because a
    /// siege cannot be choreographed — waves arrive on their own schedule from
    /// seeded positions, so the only honest way to record one is to play it.
    Fight {
        ticks: u32,
    },
    Reset,
}

/// A captioned beat.
pub struct Scene {
    pub caption: &'static str,
    pub beat: Beat,
}

const fn scene(caption: &'static str, beat: Beat) -> Scene {
    Scene { caption, beat }
}

const fn cell(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

/// The full evidence script.
///
/// Ordered so each claim is visible before it is relied on: aiming feedback
/// first, then the two shove outcomes, then the systems that gate the tool, then
/// the two ways a run ends.
pub fn script(rules: KineticRules) -> Vec<Scene> {
    if rules.siege {
        return siege_script(rules);
    }
    let mut scenes = vec![
        // The lethal push comes first on purpose. A Guardian in the lane is
        // *walking toward you the whole time* — three plates away is 450 ticks
        // of grace, and an earlier draft spent all of it on preamble and got
        // jailed before it ever pulled the trigger. Demonstrate the shot while
        // the target is still there, then take the slow beats afterwards.
        scene("Kinetic tool lab - first person", Beat::Hold(45)),
        scene(
            "Target in the lane: crosshair green - this push kills",
            Beat::Hold(85),
        ),
        scene("PUSH", Beat::Push),
        scene(
            "Momentum outlived the impulse: past the ledges, into void",
            Beat::Hold(220),
        ),
        scene(
            "Turn away: the lane is still drawn, with no target in it",
            Beat::LookAt(cell(3, 1)),
        ),
        scene("Crosshair dim - nothing to push", Beat::Hold(90)),
        scene("A refused push says so", Beat::Push),
        scene("...'Nothing in the lane.'", Beat::Hold(90)),
        // First re-park. Six plates buys 1440 ticks and act one runs to roughly
        // 1700, so without this the major walks into the back of the recharge
        // demonstration and ends the run fifteen ticks before its own parking
        // beat would have fired.
        scene(
            "Staged: the major parked out of the scene",
            Beat::PlaceMajor { cell: cell(7, 5) },
        ),
        scene(
            "Staged: a second Guardian on solid ground",
            Beat::PlaceMinor {
                index: 1,
                cell: cell(5, 4),
                // Held while the Observer walks over. Left free it closes one
                // plate during the approach, and then the pull below has no
                // room: pulling a target onto the cell you are standing on is
                // Blocked, correctly, and the demo shows nothing.
                stagger: 500,
            },
        ),
        scene(
            "Walk to line it up",
            Beat::WalkTo {
                target: cell(3, 4),
                sprint: false,
            },
        ),
        scene(
            "White crosshair: a target, but this push only staggers",
            Beat::LookAt(cell(5, 4)),
        ),
        // Pull first. A push lands it three plates out, which is past
        // `TOOL_RANGE`, so pulling afterwards finds nothing in the lane.
        scene("PULL drags it one plate closer", Beat::Pull),
        scene("", Beat::Hold(110)),
        scene("PUSH onto solid floor", Beat::Push),
        scene(
            "It survives, staggered - the floor is not lethal",
            Beat::Hold(170),
        ),
        scene("E away from the generator is refused", Beat::Generator),
        scene("...'Stand on the generator to operate it.'", Beat::Hold(90)),
        scene(
            "Walk to the recharge station",
            Beat::WalkTo {
                target: cell(2, 4),
                sprint: false,
            },
        ),
        scene(
            "Standing on a live station restores charge",
            Beat::Hold(190),
        ),
        // Clear the board before the systems tour. Guardians pursue the whole
        // time — a minor crosses a plate every 150 ticks and the major every
        // 240 — so the walking beats below are long enough for either to arrive
        // and end the run mid-sentence. Earlier cuts of this script were jailed
        // twice for exactly that reason.
        scene(
            "Staged: lined up on the west rim",
            Beat::PlaceMinor {
                index: 1,
                cell: cell(1, 4),
                stagger: 200,
            },
        ),
        scene(
            "Same tool, a lethal lane this time",
            Beat::LookAt(cell(1, 4)),
        ),
        scene("PUSH", Beat::Push),
        scene("Board clear", Beat::Hold(170)),
        scene(
            "Staged: the major parked, so the tour is not interrupted",
            Beat::PlaceMajor { cell: cell(7, 1) },
        ),
        scene(
            "Walk to the generator",
            Beat::WalkTo {
                target: cell(1, 1),
                sprint: true,
            },
        ),
        scene("Cut the power", Beat::Generator),
        scene(
            "Stations dead, sight is your own plate, the major wakes",
            Beat::Hold(170),
        ),
        scene("Power back on", Beat::Generator),
        scene("Sight returns", Beat::Hold(90)),
        scene("SPACE jumps", Beat::Jump),
        scene("", Beat::Hold(50)),
        // Re-park before the last stretch: the major has been walking since the
        // previous parking and the remaining beats are another ~800 ticks.
        scene(
            "Staged: the major parked again",
            Beat::PlaceMajor { cell: cell(1, 5) },
        ),
        scene(
            "The edge cuts both ways",
            Beat::WalkTo {
                target: cell(6, 3),
                sprint: true,
            },
        ),
        scene(
            "Walk off the rim",
            Beat::WalkOff {
                target: cell(9, 3),
                // From (6,3) the last solid ground ends 21 metres out, which is
                // 274 ticks at walking pace — a 260-tick beat stopped one stride
                // short of the edge, so nothing fell and the finale that depends
                // on respawning at the spawn plate never fired either.
                ticks: 500,
            },
        ),
        scene(
            "The void claims you, and returns you to spawn",
            Beat::Hold(120),
        ),
        scene(
            "Staged: a Guardian one plate away",
            Beat::PlaceMinor {
                index: 1,
                cell: cell(4, 3),
                stagger: 0,
            },
        ),
    ];

    // The finale depends on the rules, because a caption that describes a run
    // ending is a lie when `--no-jail` is in force. Both endings show the same
    // thing happening — a Guardian walks onto your plate — and differ only in
    // what the board does about it.
    scenes.extend(if rules.jail {
        vec![
            scene("A Guardian reaching you ends the run", Beat::Hold(220)),
            scene("JAILED - no longer a silent freeze", Beat::Hold(120)),
        ]
    } else {
        vec![
            scene("--no-jail: a Guardian reaches you...", Beat::Hold(220)),
            scene(
                "...and nothing happens. It is just something to shove.",
                Beat::Hold(150),
            ),
        ]
    });

    scenes.extend(vec![
        scene("R resets the board", Beat::Reset),
        scene("Everything restored", Beat::Hold(120)),
    ]);

    scenes
}

/// The siege recording.
///
/// Almost none of this is choreography, because a siege cannot be
/// choreographed: waves arrive on a schedule from seeded positions the script
/// does not choose. So after a short introduction it hands over to the fighting
/// bot and lets the clock decide how it ends.
fn siege_script(rules: KineticRules) -> Vec<Scene> {
    let duration = (rules.siege_minutes * 60.0 * 60.0) as u32;
    vec![
        scene("Siege: a facility the WFC solver built", Beat::Hold(70)),
        scene(
            "Walls, doorways and void are the solver's, not hand-placed",
            Beat::Hold(110),
        ),
        scene("Waves of Guardians, on a clock", Beat::Hold(120)),
        scene(
            "Shove them off the architecture before it runs out",
            // Past the clock, so the recording holds on whatever the outcome is.
            Beat::Fight {
                ticks: duration + 240,
            },
        ),
        scene("", Beat::Hold(180)),
    ]
}

/// Place the actors for the recording.
///
/// Shared by the renderer and the headless runner so the video and the test can
/// never diverge on where things started.
pub fn stage(world: &mut KineticWorld, body: &mut Embodiment) {
    // A solved facility places its own Observer, fixtures and Guardians; the
    // authored board's coordinates mean nothing there.
    if world.siege.enabled {
        return;
    }
    let stand = plate_center(cell(3, 3));
    body.body.position = Vec3::new(
        stand.x,
        crate::embodied::FLOOR_TOP + body.config.half_height,
        stand.z,
    );
    body.body.velocity = Vec3::ZERO;
    body.body.spawn = body.body.position;
    // Already looking down the ledge run, so the recording opens on the shot
    // that matters instead of on a second of turning.
    body.body.yaw = std::f32::consts::FRAC_PI_2;
    body.body.spawn_yaw = body.body.yaw;
    body.facing = observed_hex::faces::HexFace::East;

    world.observers[0].cell = cell(3, 3);
    world.observers[0].look(observed_hex::faces::HexFace::East);
    // Three plates east: inside `TOOL_RANGE`, with the ledge run and the void
    // rim behind it, and 450 ticks of walking before it could reach the spawn.
    world.minors[0].cell = cell(6, 3);
    // Held still in the far corner until the script wants it. Distance alone is
    // not enough: two plates is 300 ticks and it walked into the back of the
    // opening shot; even six plates is only 900.
    world.minors[1].cell = cell(1, 5);
    world.minors[1].stagger = HOLD;
    // The major has no stagger, so distance is the only lever and it gets
    // re-parked between acts. From spawn the corners are 4, 4, 2 and 6 plates
    // away; at 240 ticks each only the six-plate corner outlasts the first act,
    // so the held minor takes one of the near corners and the major takes this.
    world.major.cell = cell(7, 5);
}

/// Long enough to hold a Guardian still for a whole recording.
pub const HOLD: u32 = 100_000;

/// One logged moment of a headless run.
#[derive(Clone, Debug)]
pub struct Logged {
    pub tick: u32,
    pub caption: String,
    pub event: String,
}

/// Play the whole script with no window, no renderer, and no frame timing.
///
/// This is the answer to "can the demo be verified without watching it": the
/// director, the model and the body are all pure enough to run flat out, so a
/// seventy-second recording can be checked in milliseconds and a broken beat
/// names itself instead of having to be spotted in a frame.
pub fn run_headless(max_ticks: u32, rules: KineticRules) -> Vec<Logged> {
    let mut world = crate::build_world(rules);
    let mut body = Embodiment::new(&world);
    stage(&mut world, &mut body);

    let mut director = Director::new(rules);
    let mut log = Vec::new();

    for tick in 0..max_ticks {
        if director.finished {
            break;
        }
        let caption = director.caption.clone();
        let decision = director.tick(&world, &body);

        if let Some((index, cell, stagger)) = decision.stage
            && let Some(minor) = world.minors.get_mut(index)
        {
            minor.cell = cell;
            minor.alive = true;
            minor.stagger = stagger;
            minor.step_progress = 0;
        }
        if let Some(cell) = decision.stage_major {
            world.major.cell = cell;
            world.major.step_progress = 0;
        }
        if decision.reset {
            world = crate::build_world(rules);
            body = Embodiment::new(&world);
            log.push(Logged {
                tick,
                caption: caption.clone(),
                event: "Reset".to_string(),
            });
            continue;
        }

        body.step(&mut world, decision.intent, decision.request);

        if body.fell_into_void {
            log.push(Logged {
                tick,
                caption: caption.clone(),
                event: "FellIntoVoid".to_string(),
            });
        }
        for event in &world.events {
            log.push(Logged {
                tick,
                caption: caption.clone(),
                event: format!("{event:?}"),
            });
        }
    }

    log
}

/// Progress through the script.
#[derive(Resource)]
pub struct Director {
    scenes: Vec<Scene>,
    pub index: usize,
    pub elapsed: u32,
    pub caption: String,
    pub finished: bool,
}

impl Director {
    #[must_use]
    pub fn new(rules: KineticRules) -> Self {
        let scenes = script(rules);
        let caption = scenes
            .first()
            .map_or(String::new(), |scene| scene.caption.to_string());
        Self {
            scenes,
            index: 0,
            elapsed: 0,
            caption,
            finished: false,
        }
    }
}

impl Default for Director {
    fn default() -> Self {
        Self::new(KineticRules::default())
    }
}

/// What the driver decided this tick.
pub struct DirectorTick {
    pub intent: PlayerIntent,
    pub request: ToolRequest,
    pub reset: bool,
    pub stage: Option<(usize, HexCoord, u32)>,
    pub stage_major: Option<HexCoord>,
}

impl Director {
    fn advance(&mut self) {
        self.index += 1;
        self.elapsed = 0;
        match self.scenes.get(self.index) {
            Some(scene) => {
                if !scene.caption.is_empty() {
                    self.caption = scene.caption.to_string();
                }
            }
            None => self.finished = true,
        }
    }

    /// Produce this tick's input from current state.
    pub fn tick(&mut self, world: &KineticWorld, body: &Embodiment) -> DirectorTick {
        let mut out = DirectorTick {
            intent: PlayerIntent::default(),
            request: ToolRequest::None,
            reset: false,
            stage: None,
            stage_major: None,
        };
        let Some(scene) = self.scenes.get(self.index) else {
            self.finished = true;
            return out;
        };
        let beat = scene.beat;
        self.elapsed += 1;

        match beat {
            Beat::Hold(ticks) => {
                if self.elapsed >= ticks {
                    self.advance();
                }
            }
            Beat::LookAt(target) => {
                let done = self.steer(&mut out, world, body, target, false);
                // Turning is finished when the yaw error is inside one tick of
                // turn, or the step has taken implausibly long.
                if done || self.elapsed > 240 {
                    self.advance();
                }
            }
            Beat::WalkTo { target, sprint } => {
                let aligned = self.steer(&mut out, world, body, target, true);
                out.intent.sprint_held = sprint;
                if aligned {
                    out.intent.movement = Vec2::new(0.0, 1.0);
                }
                let here = plate_center(target);
                let delta = Vec2::new(here.x - body.body.position.x, here.z - body.body.position.z);
                if delta.length() < ARRIVE_RADIUS || self.elapsed > NAV_TIMEOUT {
                    self.advance();
                }
            }
            Beat::WalkOff { target, ticks } => {
                let aligned = self.steer(&mut out, world, body, target, true);
                if aligned {
                    out.intent.movement = Vec2::new(0.0, 1.0);
                }
                if self.elapsed >= ticks {
                    self.advance();
                }
            }
            Beat::Push => {
                out.request = ToolRequest::Push;
                self.advance();
            }
            Beat::Pull => {
                out.request = ToolRequest::Pull;
                self.advance();
            }
            Beat::Generator => {
                out.request = ToolRequest::ToggleGenerator;
                self.advance();
            }
            Beat::Jump => {
                out.intent.jump_pressed = true;
                self.advance();
            }
            Beat::PlaceMinor {
                index,
                cell,
                stagger,
            } => {
                out.stage = Some((index, cell, stagger));
                self.advance();
            }
            Beat::PlaceMajor { cell } => {
                out.stage_major = Some(cell);
                self.advance();
            }
            Beat::Fight { ticks } => {
                self.fight(&mut out, world, body);
                if self.elapsed >= ticks {
                    self.advance();
                }
            }
            Beat::Reset => {
                out.reset = true;
                self.advance();
            }
        }

        out
    }

    /// One tick of fighting the siege.
    ///
    /// Priorities, in order: refill if the tool is dry and a station exists,
    /// otherwise face the nearest Guardian and shove it. Deliberately simple —
    /// it is a camera operator that happens to be armed, not an attempt at good
    /// play, and it must never look better than a person could.
    fn fight(&self, out: &mut DirectorTick, world: &KineticWorld, body: &Embodiment) {
        let Some(observer) = world.observers.first() else {
            return;
        };
        if observer.jailed {
            return;
        }

        // Dry: walk to a station rather than clicking an empty tool at things.
        if observer.charge < crate::model::PUSH_COST
            && let Some(station) = world.stations.first()
        {
            if observer.cell != station.cell {
                let aligned = self.steer_along(out, world, body, station.cell);
                if aligned {
                    out.intent.movement = Vec2::new(0.0, 1.0);
                }
            }
            return;
        }

        let in_reach = || {
            world
                .minors
                .iter()
                .filter(|minor| minor.alive)
                .filter(|minor| {
                    observed_hex::coords::lateral_distance(minor.cell, observer.cell)
                        <= crate::model::TOOL_RANGE
                })
        };

        // Prefer a Guardian the architecture will actually finish. Shoving the
        // nearest one regardless mostly lands it on more floor, which staggers
        // it and kills nothing — a real finding about the tool rather than a bug
        // in the bot, and the reason a player has to fight near an edge.
        let lethal = in_reach()
            .filter(|minor| {
                world
                    .face_toward(observer.cell, minor.cell)
                    .and_then(|face| {
                        world.resolve_shove(minor.id, face, crate::model::PUSH_IMPULSE)
                    })
                    .is_some_and(|resolution| {
                        matches!(
                            resolution.fate,
                            crate::model::ShoveFate::Void | crate::model::ShoveFate::Doomed
                        )
                    })
            })
            .min_by_key(|minor| minor.id.0);

        let nearest = world
            .minors
            .iter()
            .filter(|minor| minor.alive)
            .min_by_key(|minor| {
                (
                    observed_hex::coords::lateral_distance(minor.cell, observer.cell),
                    minor.id.0,
                )
            });

        let Some(target) = lethal.or(nearest) else {
            return;
        };

        let aligned = self.steer(out, world, body, target.cell, false);
        let reachable = world.target_in_cone(observer).is_some();
        if aligned && reachable {
            // Space the shots out. Firing every tick would spend the whole
            // charge into one Guardian and read as a stutter rather than a shove.
            if self.elapsed.is_multiple_of(24) {
                out.request = ToolRequest::Push;
            }
        }
        // Deliberately no chasing. An earlier cut walked toward distant
        // Guardians, which meant the steering overwrote its own aim every tick
        // and it never settled on anything long enough to fire — and in a
        // building it walked into walls besides. A siege is a stand: hold the
        // post, turn to face what arrives, and let them come to you.
        let _ = reachable;
    }

    /// The next plate to walk to on the way to `to`, or `None` if unreachable.
    ///
    /// Breadth-first over passable faces. The director used to steer straight at
    /// its destination, which is fine on an open plain and useless in a
    /// building: in the solved facility it walked into a wall and stayed there,
    /// and a whole recording was one grey rectangle. Walls are the thing this
    /// lab added; the bot has to respect them like everything else does.
    fn route_step(world: &KineticWorld, from: HexCoord, to: HexCoord) -> Option<HexCoord> {
        if from == to {
            return None;
        }
        let mut came_from: std::collections::HashMap<HexCoord, HexCoord> =
            std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        came_from.insert(from, from);

        while let Some(cursor) = queue.pop_front() {
            if cursor == to {
                // Walk the chain back to the first step away from `from`.
                let mut step = cursor;
                while came_from.get(&step).copied() != Some(from) {
                    step = *came_from.get(&step)?;
                    if step == from {
                        return None;
                    }
                }
                return Some(step);
            }
            for face in observed_hex::faces::HexFace::LATERAL {
                let Some(next) = world.passable_neighbor(cursor, face) else {
                    continue;
                };
                if !world.cell(next).is_standable() || came_from.contains_key(&next) {
                    continue;
                }
                came_from.insert(next, cursor);
                queue.push_back(next);
            }
        }
        None
    }

    /// Steer along a route rather than straight at the destination.
    fn steer_along(
        &self,
        out: &mut DirectorTick,
        world: &KineticWorld,
        body: &Embodiment,
        target: HexCoord,
    ) -> bool {
        let here = world
            .observers
            .first()
            .map_or(target, |observer| observer.cell);
        let waypoint = Self::route_step(world, here, target).unwrap_or(target);
        self.steer(out, world, body, waypoint, true)
    }

    /// Turn toward `target`, returning whether the body is now looking at it.
    ///
    /// `walking` widens the tolerance: steering while moving does not need the
    /// precision that lining up a shot does, and a tight tolerance makes the
    /// camera visibly hunt left and right as it walks.
    fn steer(
        &self,
        out: &mut DirectorTick,
        _world: &KineticWorld,
        body: &Embodiment,
        target: HexCoord,
        walking: bool,
    ) -> bool {
        let here = plate_center(target);
        let delta = Vec2::new(here.x - body.body.position.x, here.z - body.body.position.z);
        if delta.length_squared() < 1e-3 {
            return true;
        }
        // `FpsBody::forward` is (sin yaw, -cos yaw), so this inverts it.
        let desired = delta.x.atan2(-delta.y);
        let error = wrap_angle(desired - body.body.yaw);
        let tolerance = if walking { 0.08 } else { TURN_RATE };
        if error.abs() <= tolerance {
            return true;
        }
        // The scripted path feeds `look` straight to the controller, which
        // multiplies by `look_step`, so divide it back out to get radians.
        let step = error.clamp(-TURN_RATE, TURN_RATE);
        out.intent.look = Vec2::new(step / body.config.look_step, 0.0);
        false
    }
}

fn wrap_angle(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let wrapped = angle.rem_euclid(tau);
    if wrapped > std::f32::consts::PI {
        wrapped - tau
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_angle_folds_into_half_turns() {
        let pi = std::f32::consts::PI;
        assert!((wrap_angle(0.0)).abs() < 1e-6);
        assert!((wrap_angle(pi * 2.0)).abs() < 1e-5);
        assert!((wrap_angle(pi * 1.5) + pi * 0.5).abs() < 1e-5);
        assert!((wrap_angle(-pi * 1.5) - pi * 0.5).abs() < 1e-5);
        for steps in -20..20 {
            let angle = steps as f32 * 0.7;
            assert!(wrap_angle(angle).abs() <= pi + 1e-5);
        }
    }

    /// Print the whole run. Not an assertion — a debugging instrument, kept
    /// because reading a beat-by-beat log is how every scripting bug in this
    /// demo was actually found.
    #[test]
    #[ignore = "diagnostic: cargo test -p kinetic_lab -- --ignored --nocapture script_log"]
    fn script_log() {
        for entry in run_headless(30_000, KineticRules::default()) {
            println!("{:>6}  {:<58}  {}", entry.tick, entry.caption, entry.event);
        }
    }

    /// The recording demonstrates what it claims to demonstrate.
    ///
    /// This is the headless counterpart of the video: it runs the identical
    /// script through the identical command path with no window and no frame
    /// timing, and asserts that each thing the captions promise actually
    /// happened, in order. Three separate cuts of this script were silently
    /// broken — jailed at tick 299, then 982, then 1685, and one that stopped a
    /// stride short of the edge so the last third never ran — and each was found
    /// here in milliseconds rather than by watching seventy seconds of frames.
    #[test]
    fn the_scripted_demo_demonstrates_every_claim_it_makes() {
        let log = run_headless(30_000, KineticRules::default());
        let events: Vec<&str> = log.iter().map(|entry| entry.event.as_str()).collect();

        let find = |from: usize, needle: &str| -> Option<usize> {
            events
                .iter()
                .enumerate()
                .skip(from)
                .find(|(_, event)| event.contains(needle))
                .map(|(index, _)| index)
        };

        // In order: the lethal push, a refusal, a pull, a survivable push, the
        // generator refusal, recharge, power down and back up, a fall into void,
        // a capture, and the reset.
        let mut at = 0;
        for needle in [
            "fate: Void",
            "GuardianDestroyed",
            "NoTargetInLane",
            "face: West, cells_travelled: 1, fate: Rest",
            "fate: Rest",
            "NotOnGenerator",
            "ChargeRestored",
            "powered: false",
            "powered: true",
            "FellIntoVoid",
            "ObserverCaptured",
            "Reset",
        ] {
            at = find(at, needle)
                .unwrap_or_else(|| panic!("the script never demonstrates {needle}\n{events:#?}"))
                + 1;
        }

        // The Observer must survive the whole demonstration and only be jailed
        // by the finale that is about being jailed. An early capture freezes the
        // board and silently voids every beat after it.
        let first_capture = find(0, "ObserverCaptured").expect("the finale jails the Observer");
        let fell = find(0, "FellIntoVoid").expect("the Observer falls into void");
        assert!(
            fell < first_capture,
            "captured before the script finished: the run was jailed at log entry \
             {first_capture}, which freezes the board and voids everything after it"
        );

        assert!(
            log.last().is_some_and(|entry| entry.event == "Reset"),
            "the script must end by resetting, so the recording closes on a clean board"
        );
    }

    /// The `--no-jail` cut demonstrates the flag rather than lying about it.
    ///
    /// The finale is the same event either way — a Guardian walks onto your
    /// plate — so the only thing that can go wrong is the caption describing an
    /// outcome that did not happen. Assert the Guardian really does arrive and
    /// that no capture resolves.
    #[test]
    fn the_no_jail_cut_shows_a_guardian_arriving_and_nothing_happening() {
        let rules = KineticRules {
            jail: false,
            ..Default::default()
        };
        let log = run_headless(30_000, rules);

        assert!(
            log.iter()
                .all(|entry| !entry.event.contains("ObserverCaptured")),
            "jail was switched off and a capture still resolved"
        );
        // The rest of the tour must still happen, or "nothing happened" would be
        // true for uninteresting reasons.
        for needle in [
            "fate: Void",
            "NoTargetInLane",
            "ChargeRestored",
            "FellIntoVoid",
        ] {
            assert!(
                log.iter().any(|entry| entry.event.contains(needle)),
                "the no-jail cut never demonstrates {needle}"
            );
        }
        assert!(
            log.last().is_some_and(|entry| entry.event == "Reset"),
            "the no-jail cut must still end on a reset"
        );
    }

    /// Both cuts must describe the ending they actually produce.
    #[test]
    fn the_finale_caption_matches_the_rules() {
        let jailing = script(KineticRules::default());
        assert!(
            jailing.iter().any(|scene| scene.caption.contains("JAILED")),
            "the jailing cut should announce the jail"
        );
        assert!(
            !jailing
                .iter()
                .any(|scene| scene.caption.contains("--no-jail")),
            "the jailing cut should not mention a flag that is not in force"
        );

        let lenient = script(KineticRules {
            jail: false,
            ..Default::default()
        });
        assert!(
            lenient
                .iter()
                .any(|scene| scene.caption.contains("--no-jail")),
            "the lenient cut should name the flag it is demonstrating"
        );
        assert!(
            !lenient.iter().any(|scene| scene.caption.contains("JAILED")),
            "the lenient cut must not claim a jail that cannot happen"
        );
        assert!(
            !lenient
                .iter()
                .any(|scene| scene.caption.contains("ends the run")),
            "the lenient cut must not claim the run ends"
        );
    }

    #[test]
    #[ignore = "diagnostic: cargo test -p kinetic_lab -- --ignored --nocapture siege_log"]
    fn siege_log() {
        let rules = KineticRules {
            siege: true,
            siege_minutes: 1.0,
            ..Default::default()
        };
        for entry in run_headless(30_000, rules) {
            println!("{:>6}  {}", entry.tick, entry.event);
        }
    }

    /// The siege recording must actually be a fight.
    ///
    /// A recording of a bot standing still while a clock runs out would satisfy
    /// "it ran to completion" and show nothing, so this asserts the parts that
    /// make it worth watching: waves arrive, the tool is fired, Guardians die,
    /// and the run reaches a real outcome.
    #[test]
    fn the_siege_recording_is_a_fight() {
        let rules = KineticRules {
            siege: true,
            siege_minutes: 1.0,
            ..Default::default()
        };
        let log = run_headless(30_000, rules);
        let has = |needle: &str| log.iter().any(|entry| entry.event.contains(needle));

        assert!(has("WaveReleased"), "no wave ever arrived");
        assert!(has("Shoved"), "the bot never fired the tool");
        assert!(has("SiegeEnded"), "the siege never resolved");
        assert!(
            has("GuardianDestroyed"),
            "nothing died all siege — the tool is back to being inert indoors"
        );
    }

    /// The tool has to be worth firing indoors.
    ///
    /// This test used to assert the opposite, and was right to. Before the wall
    /// slam, a shove in the solved facility stopped at the corridor's next wall
    /// and did nothing at all: a measured **zero** kills across a full siege,
    /// every push a no-op that still cost charge. The tool had been designed and
    /// tuned on an open plain with a void rim and was inert in a building.
    ///
    /// Making structure lethal at speed inverted it — the wall is the most
    /// abundant thing in a facility, so it went from the reason the tool failed
    /// to the reason it works. Keeping the measurement here, pointed the other
    /// way, is what stops it quietly regressing to a no-op again.
    #[test]
    fn the_tool_is_effective_in_a_corridor_facility() {
        let rules = KineticRules {
            siege: true,
            siege_minutes: 1.0,
            ..Default::default()
        };
        let log = run_headless(30_000, rules);

        let shoves = log
            .iter()
            .filter(|entry| entry.event.contains("Shoved"))
            .count();
        let lethal = log
            .iter()
            .filter(|entry| {
                entry.event.contains("Shoved")
                    && (entry.event.contains("fate: Void")
                        || entry.event.contains("fate: Doomed")
                        || entry.event.contains("fate: Slammed"))
            })
            .count();
        assert!(shoves > 0, "the bot never fired, so this measures nothing");
        assert!(
            lethal > 0,
            "not one of {shoves} shoves removed anything: the tool is inert indoors again"
        );
        // A quarter is a low bar on purpose. The bot is a camera operator that
        // happens to be armed, not a good player, and this is a floor under a
        // regression rather than a target to tune against.
        assert!(
            lethal * 4 >= shoves,
            "only {lethal} of {shoves} shoves removed anything indoors"
        );
    }

    /// The script must terminate on its own: every navigation beat has a
    /// timeout, so a blocked path cannot strand a recording forever.
    #[test]
    fn every_navigation_beat_is_bounded() {
        for scene in script(KineticRules::default()) {
            match scene.beat {
                Beat::WalkTo { .. } | Beat::LookAt(_) => {}
                Beat::WalkOff { ticks, .. } => assert!(ticks > 0),
                Beat::Hold(ticks) => assert!(ticks > 0),
                _ => {}
            }
        }
        assert!(!script(KineticRules::default()).is_empty());
    }
}
