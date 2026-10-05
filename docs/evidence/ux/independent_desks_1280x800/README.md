# Independent LAN desk evidence

Nine native 1280x800 screens at scale factor 1.0. See [checks](checks.txt),
[provenance](manifest.json), and the [implementation and reproduction](../../../ux/independent_lan_architect_implementation.md).

| Graphical peer | Browser | Full lobby | Match / next page |
| --- | --- | --- | --- |
| Architect | [Role choice](architect/00_browser.png) | [Desk plus three humans](architect/01_full_lobby.png) | [Live desk](architect/02_in_match.png) |
| Observer | [Role choice](observer/00_browser.png) | [Own body and separate desk](observer/01_full_lobby.png) | [Live first-person body](observer/02_in_match.png) |
| Maximum roster | [Join](roster/00_browser.png) | [Teams 1–8](roster/01_full_lobby.png) | [Teams 9–16](roster/02_roster_page_two.png) |

The first two peers run simultaneously as separate graphical processes on a real
loopback server. They enter through production semantic Join/Ready actions and
real Loading. Two further Observer connections are neutral-input transport
fixtures; only those two acknowledge preparation without a graphical worker.
Returning-player help is staged complete and Guardian pressure is disabled.

The paired match metadata shows three physical bodies, an Architect with no
embodied player or owned replay body, and zero resyncs on both graphical clients.
Capture ticks differ; each client checks its authoritative frame digest through
the normal game adapter. These are automated native checks, not human/controller
or physical-machine LAN acceptance.

Maximum-roster mode uses one graphical and thirty-one transport clients, stays in
the lobby, and visits both pages through the real Next roster action. All sixteen
body slots and sixteen desks are represented. Team controls follow their page and
Leave remains visible. This does not validate a thirty-two-client match or its
performance. JSON bounds cover semantic frontend widgets and visible text; match
controls additionally receive manual screenshot review.
