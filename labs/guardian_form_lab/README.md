# Guardian Form Lab

Candidate forms for the major Guardian (Tumbler, Plumb, Roller), each acting out
hunting, frozen by sight, frozen by an anchor, and the catch. They stand beside the
existing minor Guardian, a 1.8 m person and the facility's doorway.

The comparison, the stills, the films and the reasoning are in
[docs/guardian_forms.md](../../docs/guardian_forms.md).

```powershell
cargo dev-run -p guardian_form_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/guardian_forms"; cargo dev-run -p guardian_form_lab
```

The Tumbler was chosen as the major Guardian and is in the game. The forms live in the
shared `observed_guardian` crate, which carries their tests. Here `view.rs` draws them,
`sound.rs` says what each sounds like and when, and `capture.rs` is the evidence plan.
Films are compiled with their sound by `tools/mux_guardian_films.py`.
