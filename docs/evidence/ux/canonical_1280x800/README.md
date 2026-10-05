# Native canonical cosmetics and roster evidence

Nine native 1280x800 screenshots at scale factor 1.0, with semantic-widget and
visible-text bounds. See [checks](checks.txt), [provenance](manifest.json), and
[implementation/reproduction](../../../ux/canonical_cosmetics_roster_implementation.md).

| Screen | Evidence |
| --- | --- |
| [Cosmetics](00_cosmetics_applied.png) | Equipped comparison and match-start guidance; role-colored iris |
| [Ascent co-op](01_ascent_coop.png) | Three Observer bodies plus a separate Architect |
| [Advanced Ascent](02_ascent_advanced.png) | Three-body custom boundary and explicit bot/desk ownership |
| [Facility race](03_race_four_seats.png) | Four-body co-op retained |
| [Connected lobby](04_connected_ascent_lobby.png) | Real server, two UDP clients, claimed desk, body ownership and competing claim disabled |
| [Cobalt / Comet / Champion](05_cobalt_comet_champion.png) | Canonical blue gimbal, six-point trail and crown bars; semantic iris retained |
| [Ember / Spark / Veteran](06_ember_spark_veteran.png) | Canonical orange gimbal, three-point trail and equal bars |
| [Void / No Trail](07_void_no_trail.png) | Canonical violet gimbal and crown; no trail allocated |
| [Recorded cosmetics](08_recorded_cosmetics.png) | Replay-owned decorative look alongside recorded world/actors |

Profiles are synthetically awarded 15 wins and equipped through the progression
API. Portrait positions, camera and movement are staged with the simulation paused;
the actual canonical eye/badge/trail renderer supplies the visuals. These images do
not validate the motor or human inputs. The replay uses the existing rules/physics
recording fixture with staged Cobalt/Comet/Champion metadata.

The connected lobby is not a fabricated roster: a real ephemeral loopback server
hosts three Observer/body seats, the game joins it and a second non-graphical UDP
client claims the desk. No client readies, so the screenshot stays in the lobby.
This establishes one graphical client plus one transport peer, not two graphical
clients or physical-machine/firewall acceptance.
