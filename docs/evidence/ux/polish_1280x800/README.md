# Native cosmetics and loading evidence

Seventeen screenshots from the native game at 1280x800, scale factor 1.0.
Each has semantic-control `.bounds.json` and visible `.text-bounds.json` sidecars.
See [checks](checks.txt), [source/artifact hashes](manifest.json) and
[implementation/reproduction](../../../ux/cosmetics_loading_implementation.md).

| Screens | What is visible |
| --- | --- |
| [Defaults](00_equipped_defaults.png) | Equipped profile and selected comparison, with Equip disabled |
| [Locked Ember](01_locked_ember.png) | Locked design can be inspected; its requirement and disabled Equip remain clear |
| [Cobalt](02_cobalt_comparison.png) | Selected color changes; equipped trail and badge stay visible |
| [Comet](03_comet_comparison.png) | Trail shape compared against No Trail |
| [Champion](04_champion_comparison.png) | Badge shape compared against Rookie |
| [Equipped combination](05_equipped_combination.png) | Both previews match the equipped Cobalt/Comet/Champion profile |
| [Observer](06_observer_loading.png) | Local Ascent perspective and one-body roster |
| [Architect](07_architect_loading.png) | Architect perspective, actual body count and extra Architect per team |
| [Race](08_race_loading.png) | Facility race and Explorer wording |
| [Spectator](09_spectator_loading.png) | Watching perspective independent of a stored player seat |
| [LAN waiting](10_lan_waiting.png) | Preparing complete, waiting for everyone, Leave action available |
| [Ready](11_ready.png) | Coarse ready phase, no percentage |
| [Layout failure](12_layout_failure.png) | Concrete new-layout recovery advice and Back to Play |
| [Rematch failure](13_rematch_failure.png) | Attempt count, Retry and Back to Results |
| [LAN file mismatch](14_lan_files_mismatch.png) | Matching game version/files explained, Leave available |
| [Silent host](15_lan_host_silent.png) | Host stopped responding, reconnection advice |
| [No match](16_no_match.png) | No elapsed work, Retry disabled, Back to Play available |

The profiles are default or synthetically awarded through the progression API
(15 wins for the unlocked fixtures); the combined equipped state uses its normal
Equip API. Cosmetic selection is staged through the screen's semantic Select
action. These are representative designs: canonical match appearance is not yet
applied, as stated by the screen.

Loading screenshots use finalized launch descriptors and explicitly staged
phases/errors. Workers are removed for those layout fixtures. The LAN waiting
fixture has an unserved loopback socket and staged launch descriptor; it does not
show a connected server. Regression tests cover the real worker and UDP boundaries.
These screenshots establish layout and wording, not human input acceptance or two
connected graphical LAN clients.
