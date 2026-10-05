# Native recorded world replay evidence

Nine screens from the native game at 1280x800 and scale factor 1.0. Each has
`.bounds.json` (semantic controls) and `.text-bounds.json` (visible text). See
[validation](checks.txt) and [source/artifact provenance](manifest.json).

[Playback video](playback.mp4) shows the viewer advancing through recorded motion
and state changes. It is H.264/yuv420p at 10 encoded frames per second, 6.9 seconds.
The first render-target warmup frame is omitted; the video begins at 0.1 seconds.
The capture driver steps evenly through recorded timestamps and waits for each
GPU readback; encoding therefore stays independent of rendering speed. Raw
capture frames stay outside Git.

| Screen | Checks |
| --- | --- |
| [Follow](00_follow.png) | Physical Observer motion in an authored floor cutaway |
| [Team](01_team.png) | Team framing independent of playback time |
| [Floor](02_floor.png) | Whole-floor composition at the recorded moment |
| [Eyes](03_eyes.png) | Recorded Observer vantage with complete geometry |
| [Prison](04_prison.png) | Recorded team maze and jailed body |
| [Card event](05_card_event.png) | Legal Rogue card, target beacon and changed geometry |
| [Rogue outcome](06_rogue.png) | Final rule-owned victory and retained prison state |
| [Rewind](07_rewind.png) | The original building and actor state at time zero |
| [Guardians](08_guardians.png) | Released major/minor forms at recorded physical locations |

The fixture advances real Ascent rules and first-person physics on deterministic
bot body inputs. It stages a legal Rogue card at tick 1, releases Guardians at 60,
jails the local body at 180, corrupts another at 300 and jails the remaining loyal
bodies at 420. The rules resolve the final victory. This demonstrates recording
and presentation, not a complete human match or connected graphical LAN session.

The automated suite also runs real-UDP multi-client/late-joiner recording and
Architect-command tests. The graphical LAN and human input acceptance gates stay
open. Playback currently retains the last match in memory; it does not save files
or record audio/chat/HUD animations.

[Implementation and reproduction instructions](../../../ux/replay_implementation.md).
