# Playtest Report: `architect_lab` Step C Integration

**Worktree:** `/run/media/will/BigFastDrive/Observed 2/.claude/worktrees/playtest`  
**Branch:** `playtest/architect-step-c`  
**Lab:** `labs/architect_lab`  
**Date:** 2026-09-18  
**Author:** Playtest Agent  

---

## Executive Summary

Four features landed in `architect_lab` independently: **Step B economy** (Observer charge, kinetic shove, per-floor power, disturbance waves), **emergency requisition**, **prison self-escape**, and **unsafe falls with true-void corruption**. Every unit test passes (84/84 tests green). 

However, under end-to-end bot play (`bot_architect = true`, looping `step_beat()` across `Pocket`, `QuickClimb`, and `FullAscent`), **two of the four features are completely dead code in practice**, one feature is **99.4% jammed in an infinite loop**, and the fourth fires in a surprising edge case. Furthermore, the simulation lacks a Loyalist victory condition, meaning every single scenario inevitably resolves as `RogueVictory`.

---

## 1. What Fires and What Does Not (By Feature & Scenario)

### Summary Matrix

| Metric / Feature | Pocket (Seed 19) | QuickClimb (Seed 11) | FullAscent (Seed 7) | Total / Combined |
| :--- | :---: | :---: | :---: | :---: |
| **Duration (Beats / Ticks)** | 12 beats (720 ticks) | 15 beats (900 ticks) | 751 beats (45,060 ticks) | 778 beats (46,680 ticks) |
| **Final Match Outcome** | `RogueVictory` | `RogueVictory` | `RogueVictory` | 3/3 `RogueVictory` (100%) |
| **Cards Played (Contradictions)** | 3 (3 contradictions) | 3 (3 contradictions) | 151 (117 contradictions) | 157 cards (123 contradictions) |
| **Architect Legal Plays Empty (off-cooldown)** | 0 | 0 | 0 | 0 (off cooldown, plays always exist) |
| **Disturbance Waves Released** | L0: 2 | L0: 2, L1: 0 | L0: 37, L1: 43 | 84 waves |
| **Minor Guardians Spawned** | 2 | 2 | 123 | 127 allocated (`id` up to 1123) |
| **Peak Active Minors / Majors** | 2 minors / 1 major | 2 minors / 1 major | 123 minors / 1 major | 123 peak concurrent minors |
| **Emergency Requisitions Taken** | **0** | **0** | **0** | **0 (NEVER FIRES)** |
| **Floor Power Toggled Off** | **0 beats unpowered** | **0 beats unpowered** | **0 beats unpowered** | **0 beats (NEVER FIRES)** |
| **Power-Gating Rules Exercised** | **No** | **No** | **No** | **Never exercised** |
| **Shove Intents Evaluated** | 2 | 0 | 654 | 656 attempts |
| **Shoves Executed (Charge Spent)** | 2 (50 charge total) | 0 | 4 (100 charge total) | 6 executed shoves |
| **Shoves Jammed (`TargetNotDetected`)** | 0 | 0 | **650 (99.4%)** | 650 jammed intents |
| **Shove Outcomes: Blocked By Wall** | 2 | 0 | 4 | 6 |
| **Shove Outcomes: Void / Retraction / Displaced** | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| **Recharge Events at Powered Stations** | 0 | 0 | 4 | 4 recharge events |
| **Jail Events** | 2 | 2 | 2 | 6 capture events |
| **Prison Escape Steps Taken** | 1 | 1 | 9 | 11 steps |
| **Completed Prison Self-Escapes** | 0 (match ended) | 0 (match ended) | **1 (escaped to active)** | 1 completed self-escape |
| **Fall Landings (Lower Structure)** | 0 | 0 | 0 | 0 |
| **True-Void Corruptions** | 0 | 0 | **1 (Observer 0)** | 1 corruption |

---

### Detailed Feature Breakdown

#### Feature 1: Step B Economy
1. **Per-Observer Charge**:
   - **FIRES**: Starts at 100 on every Observer.
   - Pocket: Observers spent 25 charge each on Beat 10 (ending at 75 charge).
   - QuickClimb: Observers never spent charge (remained at 100 charge across all 15 beats / 900 ticks).
   - FullAscent: Observer 0 spent 0 charge because all 650 shove attempts failed before charge deduction. Observer 1 spent 100 charge across 4 shoves (25 each) at beats 160-164, draining to 0 charge, then successfully sought powered recharge stations and recharged back to 100 (4 recharge events).
2. **Kinetic Shove vs Minor Guardians**:
   - **FIRES WITH SEVERE BUG**:
   - Total attempts: 656.
   - Successful executions: 6 (all 6 were `ShoveOutcome::BlockedByWall`).
   - Minor Guardians destroyed (void or retraction): **0**.
   - Minor Guardians displaced: **0**.
   - Failed executions: **650 attempts failed with `ShoveError::TargetNotDetected`** in FullAscent because the behavior tree targets adjacent hex coordinates through closed partition walls.
3. **Per-Floor Power & Generators**:
   - **DOES NOT FIRE AT ALL**:
   - Floor power was unpowered for **0 beats** across all 3 scenarios (778 total beats / 46,680 ticks).
   - `power_toggles`: 0.
   - Observers executed 0 `ToggleGenerator` intents.
   - Power-gating rules (dark observation reduction, disabled door controls, disabled ascent rooms, disabled recharge stations, disabled launch pads) were **never exercised under bot play**.
4. **Disturbance Meter & Waves**:
   - **FIRES EXTENSIVELY**:
   - Disturbance accumulates heavily from card plays and contradictions (Pocket peak: 67; QuickClimb peak: 67; FullAscent peak: 97).
   - In FullAscent, 80 waves were released across levels 0 and 1, allocating 123 Minor Guardians (`next_minor_guardian_id` climbed from 1000 to 1123).
   - All 123 Minor Guardians remained active simultaneously on the map since shoves never destroyed them and no floor collapsed.

#### Feature 2: Emergency Requisition
- **DOES NOT FIRE AT ALL**:
- Taken **0 times** across all runs (0/3 modes, 0/778 beats).
- Requisition count remained 0. Hand was never refilled to 5 via requisition, and no Major Guardian was ever summoned via requisition.

#### Feature 3: Prison Self-Escape
- **FIRES (Mode Dependent)**:
- In `Pocket` and `QuickClimb`, 0 escapes completed. Both observers were captured within 1 beat of each other (Pocket: ticks 660 and 720; QuickClimb: ticks 840 and 900). Because both were jailed, the match immediately terminated with `RogueVictory`, cutting off escape.
- In `FullAscent`, **1 escape completed successfully**: Observer 0 was captured at tick 9,780, navigated 9 consecutive beats through the vertical two-floor prison core maze (ticks 9,840 to 10,260), and stepped out of the prison core at tick 10,320, transitioning `Jailed -> Active`.

#### Feature 4: Unsafe Falls with True-Void Corruption
- **FIRES (1 Corruption, 0 Fall Landings)**:
- In `FullAscent`, Observer 0 completed prison escape at tick 10,320, stepping from the immortal prison core at `(4, 5, 1)` onto facility tile `(3, 5, 1)`.
- At that exact tick (10,320), tile `(3, 5, 1)` retracted due to an unobserved contradiction that had timed out while Observer 0 was imprisoned.
- At tick 10,321, `resolve_falls` checked support for Observer 0. The tile was `HexSpace::Void`. Below `(3, 5, 1)` at level 0, there was no surviving structure (`find_lower_surviving_structure` returned `None`).
- Observer 0 fell directly into True Void and **corrupted into the Rogue AI faction** (`ObserverState::Corrupted`).
- Observer 0 remained corrupted for 579 beats (ticks 10,321 to 45,060), executing `corrupted into Rogue faction` (Hold) until match end.

---

## 2. Why: Deciding Code Paths

### 1. Why Emergency Requisition Never Fires
In [`labs/architect_lab/src/sim/behavior.rs`](labs/architect_lab/src/sim/behavior.rs#L506-L524), `ArchitectLab::legal_commands` only enumerates `ArchitectCommand::Play`:
```rust
pub fn legal_commands(&self) -> Vec<ArchitectCommand> {
    let mut out = Vec::new();
    for card in &self.deck.hand {
        for target in self.mutable_targets() {
            for rotation in 0..6 {
                let command = ArchitectCommand::Play { card: card.id, target, rotation };
                if self.refusal(command).is_none() { out.push(command); }
            }
        }
    }
    out.sort_by_key(|command| command_key(*command));
    out
}
```
`ArchitectCommand::Requisition` is **never added** to `out`. Furthermore, in `ArchitectLab::architect_intent` ([`src/sim/behavior.rs:415-503`](labs/architect_lab/src/sim/behavior.rs#L415-L503)), the AI only filters and selects commands from `self.legal_commands()`. If no command matches its heuristic filters, it falls back to:
```rust
trace.test("hold card", true);
(None, trace)
```
The Bot Architect has no logic to evaluate or emit `ArchitectCommand::Requisition`.

### 2. Why Per-Floor Power Is Never Toggled Off
1. **Initial State**: In [`labs/architect_lab/src/economy.rs:88-89`](labs/architect_lab/src/economy.rs#L88-L89), `EconomyState::new` sets `power.insert(level, true)` for all levels.
2. **Observer Behavior**: In [`labs/architect_lab/src/sim/behavior.rs:84-90`](labs/architect_lab/src/sim/behavior.rs#L84-L90), Observers only evaluate `ToggleGenerator` if the floor is already dark:
   ```rust
   if trace.test(
       "restore floor power at generator",
       !self.economy.is_powered(observer.cell.level)
           && self.economy.is_at_generator(observer.cell),
   ) {
       return (ObserverIntent::ToggleGenerator, trace);
   }
   ```
3. **Rogue Architect & Guardians**: Guardians have no generator behavior. The Rogue Architect has a `contest generator` intent ([`src/sim/behavior.rs:451-480`](labs/architect_lab/src/sim/behavior.rs#L451-L480)), but it only places a door or contradiction near the generator; it does not toggle power.
4. **Environment**: Floor collapse closes a floor, but does not turn off power.

**Conclusion**: Because power starts ON and no entity or event ever turns it OFF, power remains ON 100% of the time, leaving all power-gating mechanics completely unexercised in bot matches.

### 3. Why Kinetic Shove Fails 99.4% of the Time
In [`labs/architect_lab/src/sim/behavior.rs:48-55`](labs/architect_lab/src/sim/behavior.rs#L48-L55):
```rust
let shove_target = if self.economy.charge(id) >= SHOVE_COST {
    self.guardians.values().find(|guardian| {
        guardian.kind == GuardianKind::Minor
            && travel_distance(observer.cell, guardian.cell) == 1
    })
} else { None };
```
`travel_distance == 1` measures hexagonal coordinate distance regardless of whether a wall or closed door exists.

However, in [`labs/architect_lab/src/economy.rs:388-392`](labs/architect_lab/src/economy.rs#L388-L392), `shove()` requires:
```rust
if !self.observed.contains(&guardian_cell)
    && !self.can_detect_adjacent(observer_cell, guardian_cell)
{
    return Err(ShoveError::TargetNotDetected);
}
```
And `can_detect_adjacent` ([`src/economy.rs:347`](labs/architect_lab/src/economy.rs#L347)) requires `p_from.is_open(face) && p_to.is_open(face.opposite())`.

When a Minor Guardian is on an adjacent hex cell behind a closed wall or partition:
1. `observer_intent` selects `ObserverIntent::Shove(guardian.id)`.
2. `apply_observer_intent` calls `self.shove(id, target_guardian)`.
3. `shove` returns `Err(ShoveError::TargetNotDetected)`.
4. `apply_observer_intent` swallows the error (`let _ = self.shove(...);`).
5. Charge is not deducted; the Observer does not move.
6. On the subsequent beat, the conditions are identical: `observer_intent` selects `Shove` again.
In `FullAscent`, this locked Observer 0 into a **650-beat infinite loop**.

### 4. Why Prison Self-Escape Never Completes in `Pocket` and `QuickClimb`
In [`labs/architect_lab/src/sim.rs:494-501`](labs/architect_lab/src/sim.rs#L494-L501):
```rust
if !self.observers.is_empty()
    && self.observers.values().all(|observer| {
        observer.state == ObserverState::Jailed
            || observer.state == ObserverState::Corrupted
    })
{
    self.outcome = MatchOutcome::RogueVictory;
}
```
In 1-2 floor scenarios, once Observer 0 is captured, the lone remaining Observer is cornered almost immediately by the Major Guardian and wave-spawned Minors. Within 1 beat of Observer 0 being jailed, Observer 1 is jailed. The exact tick Observer 1 is jailed, `all(|o| o.state == Jailed)` becomes true, triggering instant `RogueVictory`. The match halts immediately, depriving jailed Observers of any opportunity to navigate the prison maze.

---

## 3. Bugs (with Reproductions)

### Bug 1: Unreachable Minor Guardian Causes Permanent Observer Shove Lock
- **Severity**: Critical (AI softlock).
- **Description**: If a Minor Guardian is at hex distance 1 behind a closed wall, an Observer with $\ge 25$ charge will select `Shove` indefinitely. Because `can_detect_adjacent` fails, `shove()` errors without deducting charge or moving the Observer, locking the Observer into an infinite loop.
- **Reproduction**:
  Run `FullAscent` (seed 7) with `bot_architect = true`. Observe Observer 0 at `HexCoord { q: 5, r: 3, level: 0 }` between ticks 10,400 and 45,060. Observer 0 attempts to shove an unreachable adjacent Minor Guardian 650 consecutive times.

### Bug 2: Missing Emergency Requisition in Bot AI and Legal Command Set
- **Severity**: Major (Feature dead code).
- **Description**: `ArchitectLab::legal_commands` never produces `ArchitectCommand::Requisition`, and `ArchitectLab::architect_intent` has no evaluation branch for requisition.
- **Reproduction**:
  Run any scenario with `bot_architect = true`. Query `sim.requisition.count` or check `sim.command_log`. It will remain 0 indefinitely.

### Bug 3: Simulation Never Turns Off Floor Power
- **Severity**: Major (Feature dead code).
- **Description**: All floors start powered. No AI role (Observer, Guardian, Architect) or environmental event ever turns power off. Observers only restore power if it is already dark. Consequently, all power-gating rules are dead code during matches.
- **Reproduction**:
  Run any scenario for 1,000 beats. Query `sim.economy.is_powered(level)`. It remains `true` on all floors for all ticks.

### Bug 4: Observers Reaching Summit Have No Victory Condition
- **Severity**: Critical (Rule violation of Section 10).
- **Description**: Section 10 of `docs/architect_ascent_design.md` specifies a "loyal victory" condition. In code, `MatchOutcome` only contains `Running` and `RogueVictory`. When both Observers reach the summit, their intent degrades to `ObserverIntent::Hold` ("watch another approach"). They stand motionless at the summit indefinitely until Guardians slowly wander over and capture them, handing the Rogue AI a victory.
- **Reproduction**:
  Run `Pocket` mode (seed 19). Observer 1 starts at the summit `(5, 4, 0)`. Observer 0 reaches `(5, 4, 0)` at Beat 2 (tick 180). Both Observers stand at the summit doing nothing from Beat 2 until Beat 10 (tick 660), when spawned Guardians capture them.

### Bug 5: Instant Rogue Victory on All-Jailed Truncates Prison Self-Escape Feature
- **Severity**: Major (Feature truncation).
- **Description**: Section 10 specifies: *"Prison maze escape, door rescue, portal rescue, corruption-adjusted summit quorum, zero-loyal elimination, loyal victory, Rogue victory... all have focused cases"*. Triggering `RogueVictory` the instant all living Observers are jailed prevents self-escape in small scenarios whenever both players are captured around the same time.
- **Reproduction**:
  Run `Pocket` or `QuickClimb`. Observer 0 is jailed at beat 11/14. Observer 1 is jailed at beat 12/15. The match terminates immediately on beat 12/15, allowing Observer 0 only 1 step of escape.

### Bug 6: Prison Exit Cell Retraction Corrupts Escaping Observer
- **Severity**: Moderate / Design oversight.
- **Description**: Cells in the facility outside the immortal prison core can retract if an unobserved contradiction times out. In `FullAscent`, the cell immediately outside the prison core exit (`(3, 5, 1)`) retracted at tick 10,320. At that exact tick, Observer 0 stepped out of the prison core onto `(3, 5, 1)`. On tick 10,321, `resolve_falls` detected `!is_supporting` over a void column, immediately corrupting the newly escaped Observer into Rogue AI.
- **Reproduction**:
  Run `FullAscent` (seed 7). At tick 10,320 Observer 0 steps to `(3, 5, 1)`. At tick 10,321 Observer 0 corrupts into `ObserverState::Corrupted`.

---

## 4. What Could Not Be Determined

1. **Multi-Floor Fall Landings on Lower Surviving Structure**:
   - In all runs, 0 fall landings occurred. Active Observers use `route()`, which only steps on solid tiles (`placement.space != Void`). Furthermore, active Observers are protected from retraction under `retraction_protected()` via `occupied()`.
   - The only fall observed occurred upon exiting the prison core onto an already-retracted tile. In that instance, the column beneath `(3, 5, 1)` had no surviving structure at level 0, triggering true-void corruption rather than a landing.
   - Whether an Observer falling from a higher floor to a lower surviving floor correctly lands, resets velocity, and continues navigation in live bot play could not be observed without manual setup.

2. **Real-Game Behavior of Power-Gating Rules**:
   - Although unit tests verify individual power-gating clauses (e.g. `economy::tests::floor_power_gates_door_operation`), the real-game behavioral interaction of darkened observation ranges, unpowered door locking, and ascent lockout during multi-agent bot competition could not be evaluated because power never toggled off.

3. **Major Guardian Spawned via Requisition**:
   - Because emergency requisition was never generated or called by the bot Architect, the live multi-agent interaction of a requisition-spawned Major Guardian on active Observer floors was unobserved in bot matches.
