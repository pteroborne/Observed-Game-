#!/usr/bin/env python3
"""Deterministic procedural audio palette for Observed 2.

Regenerates every drop-in sound slot under assets/sounds/ (see
crates/observed_assets). Design goals:

- **One timbre family per semantic family**, so every cue is identifiable by ear
  alone: UI = tiny glass ticks; movement = soft noise thumps; structure
  (door/reroute/collapse) = pneumatic + metallic; progress (keystone/exit/escape)
  = warm bell tones rising; threat (klaxon/guardian) = low, restrained, inharmonic.
- **Ambience beds are layered scenes**, not bass drones: a pink-noise room tone,
  a district voice (hum stack / wind band / water burble / metallic comb), and
  slow LFO breathing at non-integer relative rates so 16 s loops don't audibly
  cycle.
- **Loop-perfect by construction**: noise layers are built in the frequency
  domain (a finite Fourier series is exactly periodic over the buffer), tonal
  layers and LFOs are quantized to whole cycles per loop. The klaxon is a short
  1.4 s two-tone cycle that tiles seamlessly.
- **Deterministic**: every sound uses its own fixed RNG seed; rerunning the
  script produces identical files.
- **Comfort language**: nothing screams. The klaxon is short and restrained, the
  guardian is dread (a swell, not a stinger), collapse is weight, not violence.

Requires numpy and ffmpeg (WAV is written first, ffmpeg encodes Vorbis .ogg).

Usage (from the repo root):
    python tools/generate_audio.py [--out assets/sounds] [--keep-wav]
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np

SR = 44100


# --------------------------------------------------------------------------- #
# Small DSP toolkit
# --------------------------------------------------------------------------- #


def t_axis(dur: float) -> np.ndarray:
    return np.arange(int(round(dur * SR))) / SR


def quantize_freq(freq: float, loop_dur: float) -> float:
    """Snap a frequency to a whole number of cycles over the loop."""
    cycles = max(1, round(freq * loop_dur))
    return cycles / loop_dur


def sine(freq: float, dur: float, phase: float = 0.0, loop: bool = False) -> np.ndarray:
    f = quantize_freq(freq, dur) if loop else freq
    return np.sin(2 * np.pi * f * t_axis(dur) + phase)


def harmonic_stack(
    freq: float,
    dur: float,
    partials: list[tuple[float, float]],
    loop: bool = False,
    rng: np.random.Generator | None = None,
) -> np.ndarray:
    """Sum of (ratio, amplitude) partials over a fundamental."""
    out = np.zeros(int(round(dur * SR)))
    for ratio, amp in partials:
        phase = float(rng.uniform(0, 2 * np.pi)) if rng is not None else 0.0
        out += amp * sine(freq * ratio, dur, phase=phase, loop=loop)
    return out


def shaped_noise(dur: float, shape, rng: np.random.Generator) -> np.ndarray:
    """Loop-perfect noise with a magnitude response given by shape(freqs_hz).

    Built as a random-phase inverse rFFT, so the result is exactly periodic
    over the buffer — beds made from this loop seamlessly with no crossfade.
    """
    n = int(round(dur * SR))
    freqs = np.fft.rfftfreq(n, 1 / SR)
    mags = np.asarray(shape(freqs), dtype=float)
    mags[0] = 0.0  # no DC
    phases = rng.uniform(0, 2 * np.pi, len(freqs))
    spec = mags * np.exp(1j * phases)
    x = np.fft.irfft(spec, n)
    peak = np.max(np.abs(x))
    return x / peak if peak > 0 else x


def band(freqs: np.ndarray, center: float, width_oct: float) -> np.ndarray:
    """Gaussian band in log-frequency space."""
    f = np.maximum(freqs, 1e-6)
    return np.exp(-0.5 * ((np.log2(f / center)) / width_oct) ** 2)


def pink(freqs: np.ndarray) -> np.ndarray:
    f = np.maximum(freqs, 1.0)
    return 1.0 / np.sqrt(f)


def brown(freqs: np.ndarray) -> np.ndarray:
    f = np.maximum(freqs, 1.0)
    return 1.0 / f


def lfo(rate: float, dur: float, depth: float, rng: np.random.Generator) -> np.ndarray:
    """0-centered slow sine, whole cycles per loop, random phase. Returns 1+mod."""
    phase = float(rng.uniform(0, 2 * np.pi))
    return 1.0 + depth * sine(rate, dur, phase=phase, loop=True)


def env_ar(n: int, attack: float, release_tau: float) -> np.ndarray:
    """Attack ramp then exponential release."""
    t = np.arange(n) / SR
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    r = np.exp(-np.maximum(t - attack, 0) / max(release_tau, 1e-4))
    return a * r


def env_swell(n: int, rise: float, fall: float) -> np.ndarray:
    """Smooth rise to 1 then smooth fall to 0 (raised-cosine edges)."""
    t = np.arange(n) / SR
    dur = n / SR
    up = 0.5 - 0.5 * np.cos(np.pi * np.clip(t / max(rise, 1e-4), 0, 1))
    down = 0.5 - 0.5 * np.cos(
        np.pi * np.clip((dur - t) / max(fall, 1e-4), 0, 1)
    )
    return up * down


def fade_edges(x: np.ndarray, ms: float = 8.0) -> np.ndarray:
    n = min(int(SR * ms / 1000), len(x) // 2)
    if n <= 0:
        return x
    w = 0.5 - 0.5 * np.cos(np.pi * np.arange(n) / n)
    y = x.copy()
    y[:n] *= w
    y[-n:] *= w[::-1]
    return y


def comb(x: np.ndarray, freq: float, feedback: float = 0.75) -> np.ndarray:
    """Feedback comb filter — metallic resonance at freq and its harmonics."""
    delay = max(1, int(round(SR / freq)))
    y = x.copy()
    for i in range(delay, len(y)):
        y[i] += feedback * y[i - delay]
    peak = np.max(np.abs(y))
    return y / peak if peak > 0 else y


def soft_clip(x: np.ndarray, drive: float = 1.0) -> np.ndarray:
    return np.tanh(drive * x) / np.tanh(drive)


def peak_norm(x: np.ndarray, peak: float) -> np.ndarray:
    m = np.max(np.abs(x))
    return x * (peak / m) if m > 0 else x


def stereoize(mono: np.ndarray, side: np.ndarray, width: float = 0.35) -> np.ndarray:
    """Mid/side widen: shared mid, decorrelated side layer."""
    side = peak_norm(side, 1.0) * width
    left = mono + side
    right = mono - side
    return np.stack([left, right], axis=1)


# --------------------------------------------------------------------------- #
# Ambience beds (16 s loop-perfect scenes)
# --------------------------------------------------------------------------- #

BED_DUR = 32.0


def pad(
    notes: list[tuple[float, float]],
    dur: float,
    rng: np.random.Generator,
    detune_cents: float = 6.0,
    shimmer: tuple[float, float] = (0.02, 0.13),
    shimmer_depth: float = 0.45,
    wow: float = 0.08,
) -> np.ndarray:
    """A slowly evolving drone chord — the tonal FOUNDATION of every bed.

    Each note is a detuned pair (gentle chorus beating), each oscillator breathes
    on its own slow LFO, and a subtle shared phase-wobble (`wow`) gives the whole
    chord a tape-drift feel. All frequencies and rates are loop-quantized.
    """
    n = int(round(dur * SR))
    out = np.zeros(n)
    t = np.arange(n) / SR
    wow_f = quantize_freq(0.23, dur)
    wobble = wow * np.sin(2 * np.pi * wow_f * t + float(rng.uniform(0, 2 * np.pi)))
    for freq, amp in notes:
        for sign in (-1.0, 1.0):
            f = quantize_freq(freq * (2 ** (sign * detune_cents / 2400)), dur)
            rate = float(rng.uniform(*shimmer))
            depth = shimmer_depth * float(rng.uniform(0.6, 1.0))
            phase = float(rng.uniform(0, 2 * np.pi))
            osc = np.sin(2 * np.pi * f * t + phase + wobble)
            out += (amp / 2) * osc * lfo(rate, dur, depth, rng)
    return peak_norm(out, 1.0)


# ---- composition helpers (all circular, so beds stay loop-perfect) ---------- #


def crossfade_windows(n: int, k: int, xfade_s: float) -> list[np.ndarray]:
    """k gain windows partitioning the loop into equal segments with cosine
    crossfades; windows sum to 1 everywhere and wrap around the loop seam."""
    t = np.arange(n) / SR
    total = n / SR
    seg = total / k
    xf = min(xfade_s, seg / 2)
    wins = []
    for i in range(k):
        rel = (t - i * seg) % total
        w = np.zeros(n)
        rise = rel < xf
        w[rise] = 0.5 - 0.5 * np.cos(np.pi * rel[rise] / xf)
        w[(rel >= xf) & (rel < seg)] = 1.0
        fall = (rel >= seg) & (rel < seg + xf)
        w[fall] = 0.5 + 0.5 * np.cos(np.pi * (rel[fall] - seg) / xf)
        wins.append(w)
    return wins


def progression(
    chords: list[list[tuple[float, float]]],
    dur: float,
    rng: np.random.Generator,
    xfade_s: float = 3.0,
    **pad_kwargs,
) -> np.ndarray:
    """A slow chord progression: one pad per chord, cosine-crossfaded in a cycle."""
    n = int(round(dur * SR))
    wins = crossfade_windows(n, len(chords), xfade_s)
    out = np.zeros(n)
    for chord, w in zip(chords, wins):
        out += pad(chord, dur, rng, **pad_kwargs) * w
    return peak_norm(out, 1.0)


def place_circular(buf: np.ndarray, x: np.ndarray, at_s: float) -> None:
    """Add x into buf starting at at_s, wrapping past the loop seam."""
    i = int(at_s * SR) % len(buf)
    end = i + len(x)
    if end <= len(buf):
        buf[i:end] += x
    else:
        k = len(buf) - i
        buf[i:] += x[:k]
        buf[: end - len(buf)] += x[k:]


def echo_circular(buf: np.ndarray, delay_s: float, feedback: float, taps: int = 4) -> np.ndarray:
    """Feedback-delay echo computed circularly so tails wrap into the loop head."""
    out = buf.copy()
    d = int(delay_s * SR)
    for m in range(1, taps + 1):
        out += (feedback**m) * np.roll(buf, m * d)
    return out


def pluck(freq: float, decay: float = 0.8) -> np.ndarray:
    """A soft music-box note: warm partials, quick attack, long-ish die-away."""
    dur = decay * 2.2
    x = (
        sine(freq, dur)
        + 0.25 * sine(freq * 2.0, dur)
        + 0.12 * sine(freq * 2.76, dur)
    )
    return x * env_ar(len(x), 0.008, decay * 0.35)


def long_tone(freq: float, length: float) -> np.ndarray:
    """A breathy sustained melody note that swells in and out."""
    x = sine(freq, length) + 0.2 * sine(freq * 2.0, length)
    return x * env_swell(len(x), length * 0.4, length * 0.45)


def melody(
    events: list[tuple[float, float]],
    dur: float,
    style: str = "pluck",
    note_len: float = 2.0,
    echo: tuple[float, float] | None = (0.45, 0.45),
) -> np.ndarray:
    """Sparse melodic events (time_s, freq_hz) with a wraparound echo wash."""
    buf = np.zeros(int(round(dur * SR)))
    for at, freq in events:
        note = pluck(freq) if style == "pluck" else long_tone(freq, note_len)
        place_circular(buf, note, at)
    if echo is not None:
        buf = echo_circular(buf, echo[0], echo[1])
    return peak_norm(buf, 1.0)


def sub_pulse(bpm: float, dur: float, every: int = 1) -> np.ndarray:
    """A muffled heartbeat kick — felt more than heard. Beat count is forced to
    divide the loop so the pattern tiles exactly."""
    beats = max(1, round(bpm * dur / 60 / every))
    buf = np.zeros(int(round(dur * SR)))
    thump_dur = 0.22
    n = int(thump_dur * SR)
    thump = sine(58, thump_dur) * env_ar(n, 0.004, 0.07)
    for b in range(beats):
        place_circular(buf, thump, b * dur / beats)
    return peak_norm(buf, 1.0)


def make_bed(voice, seed: int, dur: float = BED_DUR) -> np.ndarray:
    """Music only, plus the faintest low room air. Stereo width comes from a
    circular Haas-delayed copy of the music itself — NOT from side noise, which
    headphones exaggerate into a wide hiss wash (the round-3 playtest lesson).
    High-band noise reads far louder than its amplitude suggests (equal-loudness),
    so texture accents live in the voices at whisper levels or not at all."""
    rng = np.random.default_rng(seed)
    mid = peak_norm(voice(rng, dur), 1.0)
    air = shaped_noise(dur, lambda f: brown(f) * band(f, 180, 1.0), rng)
    air *= lfo(0.11, dur, 0.2, rng)
    mid = peak_norm(mid + 0.02 * air, 1.0)
    side = np.roll(mid, int(0.017 * SR)) * 0.30  # loop-safe widening, zero hiss
    st = np.stack([mid + side, mid - side], axis=1)
    return peak_norm(st, 0.55)


# Note frequencies (A440 equal temperament), for readable chord spelling.
G1, A1, B1 = 49.0, 55.0, 61.7
C2, D2, DS2, E2, F2, FS2, G2, GS2, A2, AS2, B2 = (
    65.4, 73.4, 77.8, 82.4, 87.3, 92.5, 98.0, 103.8, 110.0, 116.5, 123.5,
)
C3, CS3, D3, E3, F3, FS3, G3, GS3, A3, B3 = (
    130.8, 138.6, 146.8, 164.8, 174.6, 185.0, 196.0, 207.7, 220.0, 246.9,
)
C4, CS4, D4, E4, FS4, G4, A4, B4 = (
    261.6, 277.2, 293.7, 329.6, 370.0, 392.0, 440.0, 493.9,
)
CS5, D5, E5, G5 = 554.4, 587.3, 659.3, 784.0


def voice_neutral(rng, dur):
    # Neutral facility tone: Am to Fmaj7 swaying slowly. Calm, unresolved.
    chords = [
        [(A1, 1.0), (A2, 0.5), (E3, 0.35), (C4, 0.15)],
        [(F2, 1.0), (A2, 0.5), (E3, 0.35), (C4, 0.15)],
    ]
    return progression(chords, dur, rng, xfade_s=4.0)


def voice_archive(rng, dur):
    # A half-remembered library: Amaj7 / F#m9 / Dmaj7 / Esus, with a faint
    # music-box line drifting through the stacks.
    chords = [
        [(A2, 1.0), (E3, 0.6), (GS3, 0.4), (CS4, 0.25)],
        [(FS2, 1.0), (CS3, 0.6), (A3, 0.4), (GS3, 0.2)],
        [(D3, 1.0), (A3, 0.6), (CS4, 0.35), (FS4, 0.15)],
        [(E2, 1.0), (B3, 0.5), (E4, 0.25)],
    ]
    tune = melody(
        [(2.5, CS5), (6.0, B4), (10.5, A4), (14.0, FS4), (18.5, E5), (22.0, CS5), (26.5, B4)],
        dur,
        style="pluck",
        echo=(0.5, 0.45),
    )
    dust = shaped_noise(dur, lambda f: band(f, 3000, 0.7), rng)
    dust *= lfo(0.05, dur, 0.5, rng) * lfo(0.023, dur, 0.3, rng)
    return progression(chords, dur, rng) + 0.20 * tune + 0.012 * dust


def voice_reactor(rng, dur):
    # The machine heart: an A-minor drone whose inner voice creeps E-F-E-D,
    # over a muffled 60 bpm pulse. Groove, not song.
    drone = pad(
        [(A1, 1.0), (A2, 0.6), (C3, 0.3)], dur, rng,
        detune_cents=10.0, shimmer_depth=0.3,
    )
    inner = progression(
        [[(E3, 1.0)], [(F3, 1.0)], [(E3, 1.0)], [(D3, 1.0)]],
        dur, rng, xfade_s=2.5,
    )
    core = soft_clip(1.5 * (drone + 0.35 * inner), 1.6)
    pulse = sub_pulse(60, dur)
    floor = shaped_noise(dur, lambda f: brown(f) * band(f, 70, 1.0), rng)
    return peak_norm(core, 1.0) + 0.13 * pulse + 0.04 * floor


def voice_atrium(rng, dur):
    # The empty mall atrium: Dmaj9 to Gmaj7, long breathy notes overhead.
    chords = [
        [(D3, 1.0), (A3, 0.6), (CS4, 0.35), (E4, 0.25), (FS4, 0.15)],
        [(G2, 1.0), (D3, 0.6), (B3, 0.4), (FS4, 0.15)],
    ]
    tune = melody(
        [(4.0, FS4), (12.0, A4), (20.0, E4), (27.0, D4)],
        dur,
        style="long",
        note_len=2.8,
        echo=(0.6, 0.35),
    )
    wind = shaped_noise(dur, lambda f: pink(f) * band(f, 600, 1.2), rng)
    wind *= lfo(0.045, dur, 0.45, rng) * lfo(0.017, dur, 0.25, rng)
    return progression(chords, dur, rng, xfade_s=4.0) + 0.22 * tune + 0.02 * wind


def voice_foundry(rng, dur):
    # Industry in a minor key: Fm to D#, struck metal ringing far away.
    chords = [
        [(F2, 1.0), (C3, 0.6), (GS3, 0.4)],
        [(DS2, 1.0), (AS2, 0.6), (F3, 0.35)],
    ]
    pings = np.zeros(int(round(dur * SR)))
    for at, freq in [(3.5, A3), (11.0, F3), (19.5, G3), (27.0, E3)]:
        src = pluck(freq, decay=1.2)
        ping = comb(src, freq, feedback=0.6)[: len(src)]
        place_circular(pings, peak_norm(ping, 1.0), at)
    pings = echo_circular(pings, 0.7, 0.4)
    rumble = shaped_noise(dur, lambda f: brown(f) * band(f, 110, 1.1), rng)
    return (
        progression(chords, dur, rng, shimmer_depth=0.35)
        + 0.10 * peak_norm(pings, 1.0)
        + 0.05 * rumble
    )


def voice_hollow(rng, dur):
    # The emptiest room: bare fifths drifting E-C-D-C, one lonely note per loop.
    chords = [
        [(E2, 1.0), (B2, 0.6)],
        [(C2, 1.0), (G2, 0.6)],
        [(D2, 1.0), (A2, 0.6)],
        [(C2, 1.0), (G2, 0.6)],
    ]
    lonely = melody([(13.0, B4)], dur, style="pluck", echo=(0.9, 0.55))
    sub = sine(41.2, dur, loop=True) * lfo(0.05, dur, 0.4, rng)
    return (
        progression(chords, dur, rng, shimmer=(0.015, 0.06), shimmer_depth=0.6)
        + 0.15 * lonely
        + 0.15 * sub
    )


def voice_spillway(rng, dur):
    # Water music: G to F wash, droplets landing on a pentatonic scale.
    chords = [
        [(G2, 1.0), (D3, 0.6), (A3, 0.35)],
        [(F2, 1.0), (C3, 0.6), (A3, 0.35)],
    ]
    drops = melody(
        [(1.7, D5), (5.2, G5), (9.1, E5), (13.8, D5), (17.3, A4), (21.9, G5), (25.4, E5), (29.2, B4)],
        dur,
        style="pluck",
        echo=(0.55, 0.5),
    )
    burble = shaped_noise(dur, lambda f: band(f, 1500, 0.9), rng)
    m = lfo(0.9, dur, 0.35, rng) * lfo(2.3, dur, 0.25, rng) * lfo(0.21, dur, 0.3, rng)
    return progression(chords, dur, rng, xfade_s=4.0) + 0.16 * drops + 0.035 * burble * m


def voice_corridor(rng, dur):
    # Narrow and close: Bm to G low alternation, a rare note far down the hall.
    chords = [
        [(B1, 1.0), (FS2, 0.55), (B2, 0.4), (D3, 0.25)],
        [(G1, 1.0), (G2, 0.55), (B2, 0.4), (D3, 0.25)],
    ]
    far = melody([(9.0, FS4), (24.0, D4)], dur, style="pluck", echo=(0.8, 0.5))
    duct = shaped_noise(dur, lambda f: pink(f) * band(f, 420, 0.9), rng)
    duct *= lfo(0.14, dur, 0.2, rng)
    return (
        progression(chords, dur, rng, xfade_s=4.0, shimmer_depth=0.3)
        + 0.12 * far
        + 0.018 * duct
    )


def voice_gantry(rng, dur):
    # Height and air: C / Am / F / G — the slowed mall-classic — swelling as one
    # body, long tones hanging over the drop.
    chords = [
        [(C2, 1.0), (G2, 0.65), (C3, 0.45), (E3, 0.3)],
        [(A1, 1.0), (E2, 0.65), (A2, 0.45), (C3, 0.3)],
        [(F2, 1.0), (C3, 0.65), (F3, 0.4), (A3, 0.25)],
        [(G1, 1.0), (G2, 0.65), (B2, 0.4), (D3, 0.25)],
    ]
    tune = melody(
        [(3.0, E4), (10.5, C4), (18.0, A3), (26.0, B3)],
        dur,
        style="long",
        note_len=3.2,
        echo=(0.7, 0.35),
    )
    body = progression(chords, dur, rng) * lfo(0.04, dur, 0.22, rng)
    whistle = shaped_noise(dur, lambda f: band(f, 1700, 0.35), rng)
    whistle *= lfo(0.026, dur, 0.65, rng) * lfo(0.31, dur, 0.25, rng)
    return body + 0.20 * tune + 0.01 * whistle


# --------------------------------------------------------------------------- #
# One-shot cues
# --------------------------------------------------------------------------- #


def cue_ui_hover(rng) -> np.ndarray:
    # Family UI: the smallest glass tick.
    dur = 0.07
    x = 0.7 * sine(2400, dur) + 0.4 * sine(3600, dur)
    return x * env_ar(len(x), 0.002, 0.018)


def cue_ui_click(rng) -> np.ndarray:
    # Family UI: two-step confirm tick, a third above the hover.
    n1 = sine(1800, 0.05) * env_ar(int(0.05 * SR), 0.002, 0.014)
    n2 = sine(2700, 0.07) * env_ar(int(0.07 * SR), 0.002, 0.02)
    out = np.zeros(int(0.11 * SR))
    out[: len(n1)] += n1
    out[int(0.035 * SR) : int(0.035 * SR) + len(n2)] += 0.9 * n2
    return out


def cue_footstep(rng) -> np.ndarray:
    # Family movement: soft contact thump, mostly noise.
    dur = 0.13
    pad = shaped_noise(dur, lambda f: band(f, 220, 1.0), rng)
    knock = sine(72, dur) * 0.6
    x = (0.8 * pad + knock) * env_ar(int(dur * SR), 0.003, 0.03)
    return x


def cue_jump(rng) -> np.ndarray:
    # Family movement: a quick airy lift — noise band sweeping up.
    dur = 0.22
    t = t_axis(dur)
    sweep_f = 300 * (900 / 300) ** (t / dur)
    phase = 2 * np.pi * np.cumsum(sweep_f) / SR
    tone = np.sin(phase) * 0.35
    air = shaped_noise(dur, lambda f: band(f, 800, 0.9), rng) * 0.8
    x = (tone + air) * env_swell(len(t), 0.04, 0.12)
    return x


def cue_land(rng) -> np.ndarray:
    # Family movement: the jump's other half — low thump, no sweep.
    dur = 0.18
    thump = sine(80, dur) * env_ar(int(dur * SR), 0.002, 0.05)
    grit = shaped_noise(dur, lambda f: band(f, 300, 1.0), rng)
    grit *= env_ar(int(dur * SR), 0.001, 0.02)
    return thump + 0.5 * grit


def cue_door(rng) -> np.ndarray:
    # Family structure: pneumatic hiss + low clunk + latch.
    dur = 0.45
    n = int(dur * SR)
    hiss = shaped_noise(dur, lambda f: band(f, 2000, 1.0), rng)
    hiss *= env_ar(n, 0.01, 0.10)
    clunk = sine(65, dur) * env_ar(n, 0.002, 0.07)
    latch = sine(1100, dur) * env_ar(n, 0.001, 0.008)
    latch = np.roll(latch, int(0.22 * SR))
    latch[: int(0.22 * SR)] = 0
    return 0.5 * hiss + 0.9 * clunk + 0.25 * latch


def cue_reroute(rng) -> np.ndarray:
    # Family structure: the reality shift — icy detuned shimmer gliding down.
    dur = 0.8
    t = t_axis(dur)
    out = np.zeros(len(t))
    for detune, amp in [(0.0, 1.0), (4.0, 0.6), (-3.0, 0.6), (7.0, 0.3)]:
        f = (900 + detune) * (300 / 900) ** (t / dur)
        phase = 2 * np.pi * np.cumsum(f) / SR + rng.uniform(0, 2 * np.pi)
        out += amp * np.sin(phase)
    whoosh = shaped_noise(dur, lambda f: band(f, 1200, 1.2), rng)
    x = (0.3 * peak_norm(out, 1.0) + 0.18 * whoosh) * env_swell(len(t), 0.06, 0.45)
    return x


def cue_collapse_sting(rng) -> np.ndarray:
    # Family structure: weight — rumble swell, a crack, a falling drone. No scream.
    dur = 1.6
    n = int(dur * SR)
    t = t_axis(dur)
    rumble = shaped_noise(dur, lambda f: brown(f) * band(f, 90, 1.2), rng)
    rumble *= env_swell(n, 0.25, 0.9)
    f = 90 * (55 / 90) ** (t / dur)
    drone = np.sin(2 * np.pi * np.cumsum(f) / SR)
    drone *= env_swell(n, 0.3, 1.0)
    crack = shaped_noise(0.09, lambda f: band(f, 1800, 1.4), rng)
    crack *= env_ar(len(crack), 0.001, 0.02)
    out = 0.9 * rumble + 0.5 * drone
    i = int(0.42 * SR)
    out[i : i + len(crack)] += 0.55 * crack
    return out


def cue_klaxon(rng) -> np.ndarray:
    # Family threat: SHORT two-tone cycle (1.4 s) that tiles as a loop. Restrained.
    dur = 1.4
    n = int(dur * SR)
    out = np.zeros(n)

    def tone(freq, start, length):
        seg = harmonic_stack(freq, length, [(1, 1.0), (3, 0.22), (5, 0.08)], rng=rng)
        seg *= env_swell(len(seg), 0.05, 0.08)
        i = int(start * SR)
        out[i : i + len(seg)] += seg

    tone(520, 0.00, 0.55)
    tone(390, 0.70, 0.55)
    return soft_clip(out, 1.2)


def cue_tool_interact(rng) -> np.ndarray:
    # Family tools: servo chirp + end click — "device acknowledges".
    dur = 0.25
    t = t_axis(dur)
    f = 500 + 250 * (t / dur)
    fm = 1 + 0.004 * np.sin(2 * np.pi * 42 * t)
    chirp = np.sin(2 * np.pi * np.cumsum(f * fm) / SR)
    chirp *= env_swell(len(t), 0.02, 0.10)
    click = sine(1500, dur) * env_ar(len(t), 0.001, 0.006)
    click = np.roll(click, int(0.19 * SR))
    click[: int(0.19 * SR)] = 0
    return 0.7 * chirp + 0.3 * click


def bell(freq: float, dur: float, rng) -> np.ndarray:
    """Warm bell: fundamental + slightly inharmonic upper partial."""
    n = int(dur * SR)
    x = sine(freq, dur) + 0.4 * sine(freq * 2.76, dur) + 0.2 * sine(freq * 5.4, dur)
    return x * env_ar(n, 0.004, dur * 0.28)


def cue_keystone(rng) -> np.ndarray:
    # Family progress: three warm bell notes rising (E4 G4 B4).
    dur = 0.55
    out = np.zeros(int(dur * SR))
    for i, f in enumerate([329.6, 392.0, 493.9]):
        b = bell(f, 0.3, rng)
        j = int(i * 0.09 * SR)
        out[j : j + len(b)] += b * (0.8 + 0.1 * i)
    return out


def cue_exit_unlock(rng) -> np.ndarray:
    # Family progress: the gate blooms open — an A-major chord swell + sub pulse.
    dur = 0.9
    n = int(dur * SR)
    chord = np.zeros(n)
    for f, amp in [(220.0, 1.0), (329.6, 0.7), (440.0, 0.55), (554.4, 0.35)]:
        chord += amp * (sine(f, dur) + 0.3 * sine(f * 2.76, dur))
    chord *= env_swell(n, 0.10, 0.55)
    sub = sine(55, dur) * env_ar(n, 0.01, 0.2)
    return 0.8 * peak_norm(chord, 1.0) + 0.3 * sub


def cue_escape(rng) -> np.ndarray:
    # Family progress: the biggest positive — five bells up a ladder, then a bloom.
    dur = 1.6
    out = np.zeros(int(dur * SR))
    for i, f in enumerate([329.6, 392.0, 493.9, 587.3, 659.3]):
        b = bell(f, 0.32, rng)
        j = int(i * 0.11 * SR)
        out[j : j + len(b)] += b * (0.65 + 0.08 * i)
    n = len(out)
    chord = np.zeros(n)
    for f, amp in [(329.6, 1.0), (493.9, 0.7), (659.3, 0.5)]:
        chord += amp * (sine(f, dur) + 0.25 * sine(f * 2.0, dur))
    chord *= np.roll(env_swell(n, 0.15, 0.7), int(0.55 * SR))
    shimmer = shaped_noise(dur, lambda f: band(f, 6000, 0.6), rng)
    shimmer *= env_swell(n, 0.8, 0.6)
    return out + 0.5 * peak_norm(chord, 1.0) + 0.08 * shimmer


def cue_guardian_dread(rng) -> np.ndarray:
    # Family threat: dread, not a stinger — inharmonic low cluster swelling.
    dur = 2.2
    n = int(dur * SR)
    cluster = np.zeros(n)
    for f, amp in [(55.0, 1.0), (66.5, 0.7), (82.4, 0.5)]:
        cluster += amp * sine(f, dur, phase=float(rng.uniform(0, 2 * np.pi)))
    throb = 1.0 + 0.3 * np.sin(2 * np.pi * 1.3 * t_axis(dur))
    ring = sine(1234.0, dur) * 0.05 * env_swell(n, 1.2, 0.8)
    x = (0.9 * peak_norm(cluster, 1.0) * throb + ring) * env_swell(n, 0.7, 0.9)
    return x


# --------------------------------------------------------------------------- #
# Catalogue and I/O
# --------------------------------------------------------------------------- #

# (filename, builder, seed, target peak). Peaks bake a sensible relative mix
# under the AudioDirector's cue-table volumes (which sit on top).
BEDS = [
    ("ambience.ogg", voice_neutral, 101),
    ("ambience_archive.ogg", voice_archive, 102),
    ("ambience_reactor.ogg", voice_reactor, 103),
    ("ambience_atrium.ogg", voice_atrium, 104),
    ("ambience_foundry.ogg", voice_foundry, 105),
    ("ambience_hollow.ogg", voice_hollow, 106),
    ("ambience_spillway.ogg", voice_spillway, 107),
    ("ambience_corridor.ogg", voice_corridor, 108),
    ("ambience_gantry.ogg", voice_gantry, 109),
]

CUES = [
    ("ui_hover.ogg", cue_ui_hover, 201, 0.35),
    ("ui_click.ogg", cue_ui_click, 202, 0.45),
    ("footstep.ogg", cue_footstep, 203, 0.5),
    ("jump.ogg", cue_jump, 204, 0.55),
    ("land.ogg", cue_land, 205, 0.6),
    ("door.ogg", cue_door, 206, 0.65),
    ("reroute.ogg", cue_reroute, 207, 0.7),
    ("collapse_sting.ogg", cue_collapse_sting, 208, 0.8),
    ("klaxon.ogg", cue_klaxon, 209, 0.6),
    ("tool_interact.ogg", cue_tool_interact, 210, 0.55),
    ("keystone.ogg", cue_keystone, 211, 0.65),
    ("exit_unlock.ogg", cue_exit_unlock, 212, 0.7),
    ("escape.ogg", cue_escape, 213, 0.75),
    ("guardian_dread.ogg", cue_guardian_dread, 214, 0.6),
]


def write_wav(path: Path, x: np.ndarray) -> None:
    if x.ndim == 1:
        x = x[:, None]
    data = np.clip(x, -1.0, 1.0)
    pcm = (data * 32767).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(x.shape[1])
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


def encode_ogg(wav: Path, ogg: Path, quality: int = 4) -> None:
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav),
         "-c:a", "libvorbis", "-qscale:a", str(quality), str(ogg)],
        check=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default="assets/sounds", type=Path)
    parser.add_argument("--keep-wav", action="store_true")
    args = parser.parse_args()
    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    if shutil.which("ffmpeg") is None:
        print("ffmpeg not found on PATH", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory() as tmp:
        tmpdir = Path(tmp)
        for name, voice, seed in BEDS:
            x = make_bed(voice, seed)
            wav = tmpdir / (name + ".wav")
            write_wav(wav, x)
            encode_ogg(wav, out / name, quality=3)
            if args.keep_wav:
                shutil.copy(wav, out / (name + ".wav"))
            print(f"  bed  {name:26s} {len(x) / SR:5.2f}s stereo")
        for name, builder, seed, peak in CUES:
            rng = np.random.default_rng(seed)
            x = fade_edges(peak_norm(builder(rng), peak), 4.0)
            if name == "klaxon.ogg":
                # A loop: keep the edges exactly as constructed (silence), no fade.
                x = peak_norm(builder(np.random.default_rng(seed)), peak)
            wav = tmpdir / (name + ".wav")
            write_wav(wav, x)
            encode_ogg(wav, out / name)
            if args.keep_wav:
                shutil.copy(wav, out / (name + ".wav"))
            print(f"  cue  {name:26s} {len(x) / SR:5.2f}s peak {peak}")
    print(f"wrote {len(BEDS) + len(CUES)} files to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
