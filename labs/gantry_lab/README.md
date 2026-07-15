# Gantry Lab

Resettable feasibility lab for the seeded, multidirectional Gantry expanse.

Run:

```powershell
cargo run -p gantry_lab
```

The lab renders a deterministic 128 x 96 metre course from
`observed_traversal::gantry`. Its threshold starts on a 36 metre-high platform among a
field of hexagonal megacolumns. The explicit ordered routes turn repeatedly rather
than sorting toward one axis:

- `1` jump line (amber): fast twisting high-platform route to the upper exit.
- `2` high bridge (cyan): longer connected route to the same upper exit.
- `3` understory (green): longest recovery route through the column bases to a
  distinct lower exit.
- `R` reset the current route.

Purple markers are threshold endpoints and the white line is the active runner trail.
Success criteria are unit-tested in `observed_traversal`: the footprint and deck
height are pinned, the jump route has at least twelve nodes and four heading changes,
the three controller runs preserve fast/medium/slow timing, and generation/replay is
deterministic across seeds.
