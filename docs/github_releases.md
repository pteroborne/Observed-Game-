# GitHub Releases

The repository produces a native Steam Deck/Linux package using Valve's
[Steam Linux Runtime 4 SDK](https://github.com/ValveSoftware/steam-runtime#introduction).
The package also runs on compatible x86_64 desktop Linux systems.

## Test a release build

Run the **Steam Deck release** workflow manually from GitHub's Actions page. A manual
run builds and tests the game, then stores the archive and SHA-256 checksum as a
14-day workflow artifact. It does not create a public GitHub Release.

The packaged layout is:

```text
observed-<version>-steam-deck-linux-x86_64/
|-- observed
|-- README.txt
`-- assets/
```

The executable resolves `assets/` beside itself. `OBSERVED2_ASSET_ROOT` can override
that location for diagnostics or a custom launcher.

## Publish a release

After the manual artifact has been exercised on a Steam Deck:

1. Choose a version such as `0.1.0`.
2. Create and push an annotated `v0.1.0` tag on the verified commit.
3. Wait for the **Steam Deck release** workflow to finish.
4. Confirm that the GitHub Release contains both the `.tar.gz` archive and its
   `.sha256` checksum.
5. Download the public asset once and repeat the Deck smoke test.

Rerunning a tag workflow replaces its release assets instead of creating a duplicate
release. Avoid moving a published tag; use a new patch version after users can have
downloaded an artifact.

## What the workflow pins

- Rust `1.96.0`.
- The exact Steam Linux Runtime 4 SDK image digest.
- Exact commits for the GitHub-maintained checkout, cache, upload, and download
  actions.
- Rust dependencies through `Cargo.lock` and `--locked`.

When intentionally updating the Rust toolchain or Steam runtime image, run the
workflow manually and complete the Deck checklist before publishing the next tag.

## Steam Deck smoke checklist

- The game launches in Gaming Mode without Proton forced.
- The full menu can be navigated with the built-in controls.
- Movement, camera, jump, sprint, interaction, lantern, map, pause, and back work.
- Audio output works after suspend/resume.
- UI remains legible at 1280 x 800.
- A solo match reaches Results and can start another match.
- Settings and profile progress survive a restart.
- Frame pacing is acceptable in dense rooms, during facility mutation, and with fog
  and bloom active.

## Steamworks follow-up

GitHub Releases do not configure a Steam depot. For a Steam store build, use this
same package as the Linux depot content, configure `observed` as the Linux launch
option, and select Steam Linux Runtime 4 for the branch in Steamworks.
