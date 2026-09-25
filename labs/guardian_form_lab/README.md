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

`form.rs` and `roll.rs` are the pure core (parts, poses, the octahedron's roll) and
carry the tests; `view.rs` draws them; `capture.rs` is the evidence plan.
