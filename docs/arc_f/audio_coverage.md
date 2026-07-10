# Audio Coverage Audit

Phase 56 ruling: every gameplay-critical signal either maps to a semantic cue or has
an explicit silence ruling. Normal play remains HUD-free; this is the audio companion
to the diegetic visual/tac-map reads.

| Signal | Cue / ruling | Spatial class | Notes |
| --- | --- | --- | --- |
| District identity and baseline room tone | `MatchAudioCue::Ambience` | none, music bus | Stable loop beds cross-fade by location and never restart during play: rooms take their district's bed; hallways take the corridor bed, or the gantry bed when the hall has raised decks. `fade_ambience_beds` is the only volume writer for bed sinks (the director skips them). |
| Player movement cadence | `MatchAudioCue::Footstep` | local near rolloff | Own footsteps are same-place only and suppressed when grounded movement is idle. |
| Door threshold observed/frozen by crossing | `MatchAudioCue::Door` | structural threshold | The door thunk is a local threshold cue on place transitions. |
| Team escape / placement success | `MatchAudioCue::Escape` | none | A positive sting, ducking ambience without restarting it. |
| Decoherence / unobserved reroute commit | `MatchAudioCue::Reroute` | structural threshold | Fired once per reroute burst by the decoherence feedback driver. |
| Rival enters or changes a neighbouring room | `MatchAudioCue::RivalBleed` | rival far rolloff + threshold/wall occlusion | Uses the footstep asset through the director; sighting ledger records Heard. Rival leaving is intentionally silent so absence does not become a false signal. |
| UI selection confirm | `MatchAudioCue::UiClick` | none | UI bus, available outside Match through the fallback path. |
| UI highlight movement | `MatchAudioCue::UiHover` | none | UI bus, available outside Match through the fallback path. |
| Jump start / gantry commitment | `MatchAudioCue::Jump` | local near rolloff | Fired by traversal crossing feedback. |
| Landing / fall recovery | `MatchAudioCue::Land` | local near rolloff | Edge-triggered by grounded transitions. |
| First-escape countdown pressure | `MatchAudioCue::Klaxon` | none | Single loop while the countdown exists; ambience ducked. |
| Collapse frontier reaches the current place | `MatchAudioCue::CollapseSting` | structural threshold | One warning per dying place, not per frame. |
| Anchor torch or teleport pad picked, placed, or linked | `MatchAudioCue::ToolInteract` | local near rolloff | Acknowledges successful local tool actions; anchor expiry/banish remains intentionally silent because the visual/tac-map state is the durable read. |
| Keystone pickup | `MatchAudioCue::Keystone` | local near rolloff | Fired only when an uncollected keystone is actually collected. |
| Exit unlock after final keystone | `MatchAudioCue::ExitUnlock` | none | Confirms the gate flipped open; the locked/open exit read remains visual on the tac-map and door. |
| Guardian proximity / alert dread | `MatchAudioCue::GuardianDread` | guardian dread rolloff + heavy occlusion | A low, cooldown-limited cue for same-room or adjacent guardian presence; no jump-scare stingers. |

## Explicit Silence Rulings

| Signal | Ruling |
| --- | --- |
| Anchor expiry / guardian banish timeout | Silent; the visible tether state, monitor read, and room graph are the authoritative signals. |
| Rival leaves a neighbouring room | Silent; removing/aging the sighting on the tac-map is enough, and a departure sound would over-state uncertain information. |
| Exit remains locked after a partial keystone pickup | Silent; the pickup cue already fired, while the locked exit stays red/closed. |
| Countdown unchanged between ticks | Silent; the klaxon loop owns this continuous state. |
