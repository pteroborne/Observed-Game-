# content_baker

Deterministic TrenchBroom/Quake `.map` to Observed 2 convex-collision baker.

Each selected `observed_module` entity becomes one `BakedModule`; every brush is
projected to a stable convex hull, validated by `observed_content`, sorted
canonically, and written as JSON. The committed production invocation is:

```powershell
cargo run -p content_baker -- `
  labs/rapier_portal_lab/assets/maps/portal_modules.map `
  assets/content/modules
```

The source currently emits both `gantry.json` and `colonnade.json` from their typed
`observed_module` entities.
The manifest stores SHA-256 fingerprints of both the source map and baked output;
runtime code never watches or reparses editor source files.
