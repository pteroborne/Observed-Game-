# Kinetic sound palette

19 original, procedurally synthesized cues. No sampled recordings or third-party
sound assets. Source: `tools/generate_kinetic_audio.py`, using only Python's standard
library and FFmpeg's Vorbis encoder. Each cue has a fixed seed. `manifest.json`
records source duration, peak and RMS levels. Playback/mixing belongs to
`kinetic_lab::sound`, shared with the WFC kinetic lab.

Regenerate from the repository root:

```sh
python tools/generate_kinetic_audio.py
```

Audition without rewriting the shipped files:

```sh
python tools/generate_kinetic_audio.py --out /tmp/kinetic-audio --preview /tmp/kinetic-audio/audition.wav
```

The optional preview also writes `audition.srt` with cue names and timings. Source
PCM is deterministic; Ogg container identifiers may differ between encodes.
