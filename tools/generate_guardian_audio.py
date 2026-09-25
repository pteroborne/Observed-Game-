#!/usr/bin/env python3
"""Original major-Guardian sound palette. Stdlib synthesis + ffmpeg Vorbis encoder.

No recordings or borrowed assets. Fixed per-cue seeds, 48 kHz mono, peak headroom,
short fade edges on one-shots. Loops have no fades: their tails are crossfaded into
their heads, so they repeat without a seam.

Dark and crunchy, not bright. Nothing is struck high: a tick is a low knock with a
burst of band-limited grit, not a sine ping; hiss is dark rush, never white noise;
bronze rings an octave or more down. Every cue is driven into a soft saturator for grit
and then low-passed, so the harmonics the saturation adds stay warm instead of fizzing.

The Tumbler's sounds carry its states by ear: hunting is a bronze drone under a
grinding, turning ratchet; being seen cuts it with a latch, so silence means frozen;
being let go unwinds back into the hum; an anchor clamps on in the lantern's darker
glass, not the Guardian's bronze; a catch rises and stamps. The Plumb and Roller have
their own palettes for the form lab.

--preview writes a labelled audition reel.
"""
import argparse
import array
import json
import math
from pathlib import Path
import random
import subprocess
import tempfile
import wave

RATE = 48000
TAU = math.tau

# name: (duration seconds, peak, loop, saturation drive, low-pass cutoff Hz)
CUES = {
    'tumbler_hum': (4.0, 0.30, True, 2.6, 1300),
    'tumbler_latch': (0.95, 0.45, False, 3.0, 1500),
    'tumbler_release': (0.85, 0.38, False, 2.6, 1300),
    'tumbler_clamp': (1.25, 0.40, False, 1.8, 2200),
    'tumbler_catch': (1.9, 0.46, False, 3.2, 1400),
    'plumb_hum': (4.0, 0.26, True, 1.8, 900),
    'plumb_seal': (0.95, 0.40, False, 2.0, 1700),
    'plumb_catch': (1.9, 0.46, False, 3.0, 1400),
    'roller_fall': (0.55, 0.44, False, 3.0, 1400),
    'roller_balance': (1.2, 0.34, False, 1.3, 1500),
    'roller_catch': (1.7, 0.46, False, 3.0, 1400),
}

# The Tumbler's ratchet: one tick every this many seconds while hunting, and the gear
# teeth grinding under it. Both divide the 4 s loop exactly.
TICK = 4.0 / 21
TOOTH = 4.0 / 96


class Noise:
    """Seeded noise in two dark bands: `low`, a rumble under about 300 Hz, and
    `grit`, the band between that and about 1.2 kHz, where a crunch lives."""

    def __init__(self, rng):
        self.rng = rng
        self.low = 0.0
        self.mid = 0.0

    def step(self):
        white = self.rng.uniform(-1, 1)
        self.low += 0.04 * (white - self.low)
        self.mid += 0.15 * (white - self.mid)
        return self.low, (self.mid - self.low) * 2.5


def knock(t, at, freq, rate):
    """A low knock: a thump whose pitch drops as it dies. The tick of a heavy ratchet."""
    d = t - at
    if d < 0:
        return 0.0
    f = freq * (1.0 + 1.2 * math.exp(-d * 40))
    return math.sin(TAU * f * d) * math.exp(-d * rate) * min(1.0, d * 2500)


def grain(t, at, rate, grit):
    """A burst of grit, decaying at `rate`: the crunch in a knock."""
    d = t - at
    if d < 0:
        return 0.0
    return grit * math.exp(-d * rate) * min(1.0, d * 4000)


def bronze(t, at, base, rate):
    """A dark bronze ring: three inharmonic partials, the upper ones quiet."""
    d = t - at
    if d < 0:
        return 0.0
    ring = (math.sin(TAU * base * d)
            + 0.45 * math.sin(TAU * base * 2.51 * d)
            + 0.15 * math.sin(TAU * base * 4.14 * d))
    return ring * math.exp(-d * rate) * min(1.0, d * 2000)


def thump(t, at, freq, rate):
    """A heavy low thump whose pitch drops as it decays."""
    d = t - at
    if d < 0:
        return 0.0
    f = freq * (1.0 + 1.5 * math.exp(-d * 30))
    return math.sin(TAU * f * d) * math.exp(-d * rate) * min(1.0, d * 1500)


def crunch(t, at, freq, grit, level=1.0):
    """A ratchet tick: a knock and its grit."""
    return level * (0.6 * knock(t, at, freq, 55) + 0.5 * grain(t, at, 70, grit))


def saturate(samples, drive):
    """Soft clipping for grit: tanh, scaled so full scale stays full scale."""
    peak = max(abs(s) for s in samples) or 1.0
    norm = math.tanh(drive)
    return [math.tanh(drive * s / peak) / norm for s in samples]


def lowpass(samples, cutoff):
    """Two one-pole low-passes in series: a gentle 12 dB/octave roll-off."""
    a = 1.0 - math.exp(-TAU * cutoff / RATE)
    for _ in range(2):
        y = 0.0
        out = []
        for s in samples:
            y += a * (s - y)
            out.append(y)
        samples = out
    return samples


def synth(name, duration, seed):
    rng = random.Random(seed)
    _, _, loop, drive, cutoff = CUES[name]
    # Loops are made long by half a second, which is crossfaded into their head.
    extra = 0.5 if loop else 0.0
    count = round(RATE * (duration + extra))
    ticks = []
    teeth = []
    if name == 'tumbler_hum':
        ticks = [(k * TICK + rng.uniform(-0.012, 0.012), rng.uniform(0.7, 1.0))
                 for k in range(-1, 26)]
        teeth = [(k * TOOTH + rng.uniform(-0.004, 0.004), rng.uniform(0.3, 1.0))
                 for k in range(-1, 110)]
    rattle = [rng.uniform(0.0, 0.05) for _ in range(8)]
    noise = Noise(rng)
    samples = []
    phase = 0.0
    for i in range(count):
        t = i / RATE
        x = t / duration
        low, grit = noise.step()
        if name == 'tumbler_hum':
            # Bronze drone on 55 Hz, its fifth and octave, breathing twice per loop,
            # under grinding teeth and a turning ratchet.
            breath = 0.75 + 0.25 * math.sin(TAU * 0.5 * t)
            s = breath * (0.6 * math.sin(TAU * 55 * t) + 0.3 * math.sin(TAU * 82.5 * t)
                          + 0.15 * math.sin(TAU * 110 * t))
            s += 0.6 * low * breath
            for at, level in teeth:
                s += 0.18 * level * grain(t, at, 140, grit)
            for at, level in ticks:
                s += crunch(t, at, 150, grit, 0.9 * level)
        elif name == 'tumbler_latch':
            # The ratchet runs faster and faster, then the latch drops home.
            s = 0.0
            for k in range(9):
                at = 0.2 * (1 - (1 - k / 9) ** 1.8)
                s += crunch(t, at, 170 + 12 * k, grit, 0.8)
            s += 1.3 * thump(t, 0.22, 52, 14)
            s += 1.1 * grain(t, 0.22, 22, grit) + 0.5 * knock(t, 0.22, 240, 30)
            s += 0.4 * bronze(t, 0.22, 110, 4.5)
            s += 0.7 * low * math.exp(-max(0.0, t - 0.22) * 16) * (t > 0.22)
        elif name == 'tumbler_release':
            # The latch lifts, and the ratchet winds back up into the hum.
            s = 0.7 * crunch(t, 0.02, 200, grit) + 0.5 * thump(t, 0.02, 70, 26)
            phase += TAU * (32 + 23 * x) / RATE
            s += 0.45 * math.sin(phase) * min(1.0, x * 2) * (1 - 0.3 * x)
            s += 0.3 * low * min(1.0, x * 2)
            k, at = 0, 0.12
            while at < duration - 0.02:
                s += crunch(t, at, 140 + 40 * x, grit, 0.75)
                k += 1
                at += 0.16 * (1 - 0.55 * min(1.0, k / 8))
        elif name == 'tumbler_clamp':
            # The anchor's voice: a falling rush, a magnetic thunk, a dark glass dyad.
            s = 0.8 * low * max(0.0, 1 - t / 0.3) * min(1.0, t * 40)
            s += 1.2 * thump(t, 0.3, 48, 11) + 0.6 * grain(t, 0.3, 30, grit)
            d = t - 0.3
            if d > 0:
                shimmer = 1 + 0.003 * math.sin(TAU * 4 * d)
                s += (0.3 * math.sin(TAU * 370 * shimmer * d)
                      + 0.2 * math.sin(TAU * 554 * d)) * math.exp(-d * 2.8) * min(1.0, d * 300)
        elif name == 'tumbler_catch':
            # The tiers telescope up, then the stamp: a sub boom, a crunch, and a low
            # tritone ring.
            rise = min(1.0, t / 0.45)
            phase += TAU * (30 + 150 * rise ** 2) / RATE
            s = 0.4 * math.sin(phase) * (t < 0.5) * min(1.0, t * 20)
            for k in range(12):
                s += crunch(t, 0.035 * k, 160 + 10 * k, grit, 0.7)
            s += 1.6 * thump(t, 0.5, 38, 6.5) + 1.2 * grain(t, 0.5, 14, grit)
            s += 0.9 * low * math.exp(-max(0.0, t - 0.5) * 8) * (t > 0.5)
            s += 0.35 * bronze(t, 0.5, 110, 2.4) + 0.25 * bronze(t, 0.5, 155, 2.7)
        elif name == 'plumb_hum':
            # A low pale drone, with three orbits rushing past at their own rates.
            s = 0.45 * math.sin(TAU * 82.5 * t) + 0.22 * math.sin(TAU * 123.75 * t)
            for rate, width in ((0.75, 1.2), (1.0, 1.0), (1.25, 0.8)):
                s += width * low * (0.5 + 0.5 * math.sin(TAU * rate * t)) ** 3
            s *= 0.8
        elif name == 'plumb_seal':
            # Three rings settle, high to low, and the point sets down with a crunch.
            s = 0.0
            for k, (at, f) in enumerate(((0.0, 330), (0.09, 262), (0.18, 220))):
                s += (0.5 - 0.08 * k) * bronze(t, at, f, 7)
            s += 0.9 * thump(t, 0.3, 80, 20) + 0.5 * grain(t, 0.3, 40, grit)
        elif name == 'plumb_catch':
            # Six faces grind open, the core swells, and the stamp.
            s = 0.0
            for k in range(6):
                s += crunch(t, 0.05 + 0.06 * k + rattle[k], 190 - 10 * k, grit, 0.7)
            phase += TAU * (60 + 100 * min(1.0, x * 2)) / RATE
            s += 0.4 * math.sin(phase) * math.sin(math.pi * min(1.0, x * 1.6))
            s += 1.4 * thump(t, 0.5, 42, 7.5) + 1.0 * grain(t, 0.5, 16, grit)
            s += 0.3 * bronze(t, 0.5, 165, 3)
        elif name == 'roller_fall':
            # A hollow cage landing on a face: a thud, and its struts crunching.
            s = 1.3 * thump(t, 0.0, 52, 13) + 0.9 * low * math.exp(-t * 16)
            for k in range(5):
                s += crunch(t, 0.01 + rattle[k], 240 + 60 * k, grit, 0.55)
        elif name == 'roller_balance':
            # It tips onto one point, and a low pure tone holds, as if nothing should.
            s = 0.7 * thump(t, 0.0, 70, 18) + 0.4 * grain(t, 0.0, 45, grit)
            d = t - 0.12
            if d > 0:
                s += 0.45 * math.sin(TAU * 261.6 * d) * min(1.0, d * 8) * math.exp(-d * 1.2)
                s += 0.15 * math.sin(TAU * 130.8 * d) * min(1.0, d * 8) * math.exp(-d * 1.0)
        elif name == 'roller_catch':
            # The halves split with a rush and a grinding clank, and the stamp.
            s = 0.9 * low * math.exp(-t * 5) * min(1.0, t * 60)
            s += crunch(t, 0.15, 220, grit, 1.2) + 1.4 * thump(t, 0.4, 40, 7.5)
            s += 1.0 * grain(t, 0.4, 16, grit) + 0.28 * bronze(t, 0.4, 131, 3.0)
        else:
            raise ValueError(name)
        samples.append(s)
    # Grit, then warmth: saturate, and low-pass what the saturation adds.
    samples = lowpass(saturate(samples, drive), cutoff)
    if extra:
        # Crossfade the tail into the head: the loop point is the same sound.
        n = round(RATE * duration)
        fade = count - n
        looped = samples[:n]
        for i in range(fade):
            w = i / fade
            looped[i] = samples[n + i] * (1 - w) + samples[i] * w
        samples = looped
    else:
        samples = [s * min(1.0, (i / RATE) / 0.004, (duration - i / RATE) / 0.03)
                   for i, s in enumerate(samples)]
    mean = sum(samples) / len(samples)
    samples = [s - mean for s in samples]
    peak = CUES[name][1]
    scale = peak / max(abs(s) for s in samples)
    return [s * scale for s in samples]


def write_wav(path, samples):
    pcm = array.array('h', (round(max(-1., min(1., s)) * 32767) for s in samples))
    import sys
    if sys.byteorder != 'little':
        pcm.byteswap()
    with wave.open(str(path), 'wb') as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(RATE)
        f.writeframes(pcm.tobytes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path,
                        default=Path(__file__).resolve().parents[1] / 'assets/sounds/guardian')
    parser.add_argument('--preview', type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    report = {}
    reel = []
    with tempfile.TemporaryDirectory() as temp:
        for index, (name, (duration, _peak, loop, _drive, _cutoff)) in enumerate(CUES.items()):
            samples = synth(name, duration, 7300 + index)
            assert all(math.isfinite(s) for s in samples)
            assert max(abs(s) for s in samples) <= 0.5
            if loop:
                # The seam: the last and first samples must be neighbours.
                assert abs(samples[-1] - samples[0]) < 0.05, name
            wav = Path(temp) / (name + '.wav')
            write_wav(wav, samples)
            subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', str(wav),
                            '-c:a', 'libvorbis', '-q:a', '5', str(args.out / (name + '.ogg'))],
                           check=True)
            rms = math.sqrt(sum(s * s for s in samples) / len(samples))
            report[name] = {'duration': duration, 'loop': loop,
                            'peak_dbfs': round(20 * math.log10(max(abs(s) for s in samples)), 2),
                            'rms_dbfs': round(20 * math.log10(rms), 2)}
            reel.extend(samples if not loop else samples * 2)
            reel.extend([0.] * round(RATE * 0.5))
        if args.preview:
            args.preview.parent.mkdir(parents=True, exist_ok=True)
            write_wav(args.preview, reel)
    (args.out / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Generated {len(report)} original cues in {args.out}')


if __name__ == '__main__':
    main()
