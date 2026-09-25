#!/usr/bin/env python3
"""Original major-Guardian sound palette. Stdlib synthesis + ffmpeg Vorbis encoder.

No recordings or borrowed assets. Fixed per-cue seeds, 48 kHz mono, peak headroom,
short fade edges on one-shots. Loops have no fades: their tails are crossfaded into
their heads, so they repeat without a seam.

The Tumbler's sounds carry its states by ear, as the design asks of every critical
state: hunting is a bronze hum under a turning ratchet; being seen cuts it with a
latch, so silence means frozen; being let go unwinds back into the hum; an anchor
clamps on in the lantern's glassy voice, not the Guardian's bronze; a catch rises and
stamps. The Plumb and Roller have their own palettes for the form lab.

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

# name: (duration seconds, peak, loop)
CUES = {
    'tumbler_hum': (4.0, 0.30, True),
    'tumbler_latch': (0.95, 0.45, False),
    'tumbler_release': (0.85, 0.38, False),
    'tumbler_clamp': (1.25, 0.40, False),
    'tumbler_catch': (1.9, 0.46, False),
    'plumb_hum': (4.0, 0.26, True),
    'plumb_seal': (0.95, 0.40, False),
    'plumb_catch': (1.9, 0.46, False),
    'roller_fall': (0.55, 0.44, False),
    'roller_balance': (1.2, 0.34, False),
    'roller_catch': (1.7, 0.46, False),
}

# The Tumbler's ratchet: one tick every this many seconds while hunting.
TICK = 4.0 / 21


def click(t, at, freq, rate):
    """A struck click at time `at`: a decaying partial, silent before it."""
    d = t - at
    if d < 0:
        return 0.0
    return math.sin(TAU * freq * d) * math.exp(-d * rate) * min(1.0, d * 3000)


def bronze(t, at, base, rate):
    """A bronze ring: three inharmonic partials, struck at `at`."""
    d = t - at
    if d < 0:
        return 0.0
    ring = (math.sin(TAU * base * d)
            + 0.55 * math.sin(TAU * base * 2.51 * d)
            + 0.30 * math.sin(TAU * base * 4.14 * d))
    return ring * math.exp(-d * rate) * min(1.0, d * 2000)


def thump(t, at, freq, rate):
    """A heavy low thump whose pitch drops as it decays."""
    d = t - at
    if d < 0:
        return 0.0
    f = freq * (1.0 + 1.5 * math.exp(-d * 30))
    return math.sin(TAU * f * d) * math.exp(-d * rate) * min(1.0, d * 1500)


def synth(name, duration, seed):
    rng = random.Random(seed)
    # Loops are made long by half a second, which is crossfaded into their head.
    extra = 0.5 if CUES[name][2] else 0.0
    count = round(RATE * (duration + extra))
    ticks = []
    if name == 'tumbler_hum':
        ticks = [(k * TICK + rng.uniform(-0.012, 0.012), rng.uniform(0.7, 1.0))
                 for k in range(-1, 26)]
    rattle = [rng.uniform(0.0, 0.05) for _ in range(8)]
    samples = []
    low = 0.0
    phase = 0.0
    for i in range(count):
        t = i / RATE
        x = t / duration
        noise = rng.uniform(-1, 1)
        low += 0.04 * (noise - low)
        air = noise - low
        if name == 'tumbler_hum':
            # Bronze drone on 55 Hz and its fifth, breathing twice per loop, under a
            # turning ratchet.
            breath = 0.75 + 0.25 * math.sin(TAU * 0.5 * t)
            s = breath * (0.55 * math.sin(TAU * 55 * t) + 0.3 * math.sin(TAU * 82.5 * t)
                          + 0.12 * math.sin(TAU * 165 * t))
            s += 0.5 * low * breath
            for at, level in ticks:
                s += level * (0.35 * click(t, at, 1760, 70) + 0.25 * click(t, at, 523, 45))
        elif name == 'tumbler_latch':
            # The ratchet runs faster and faster, then the latch drops home.
            s = 0.0
            for k in range(9):
                at = 0.2 * (1 - (1 - k / 9) ** 1.8)
                s += 0.3 * click(t, at, 1500 + 90 * k, 80)
            s += 1.2 * thump(t, 0.22, 68, 16)
            s += 0.6 * click(t, 0.22, 740, 40) + 0.45 * click(t, 0.22, 1113, 55)
            s += 0.35 * bronze(t, 0.22, 220, 5.0)
            s += 0.6 * low * math.exp(-max(0.0, t - 0.22) * 20) * (t > 0.22)
        elif name == 'tumbler_release':
            # The latch lifts, and the ratchet winds back up into the hum.
            s = 0.5 * click(t, 0.02, 900, 60) + 0.4 * thump(t, 0.02, 90, 30)
            phase += TAU * (35 + 25 * x) / RATE
            s += 0.45 * math.sin(phase) * min(1.0, x * 2) * (1 - 0.3 * x)
            k, at = 0, 0.12
            while at < duration - 0.02:
                s += 0.28 * click(t, at, 1500 + 200 * x, 75)
                k += 1
                at += 0.16 * (1 - 0.55 * min(1.0, k / 8))
        elif name == 'tumbler_clamp':
            # The anchor's voice: a falling hiss, a magnetic thunk, a glassy dyad.
            s = 0.25 * air * max(0.0, 1 - t / 0.3) * min(1.0, t * 40)
            s += 1.1 * thump(t, 0.3, 58, 12)
            d = t - 0.3
            if d > 0:
                shimmer = 1 + 0.004 * math.sin(TAU * 6 * d)
                s += (0.35 * math.sin(TAU * 988 * shimmer * d)
                      + 0.28 * math.sin(TAU * 1480 * d)) * math.exp(-d * 3.2) * min(1.0, d * 400)
        elif name == 'tumbler_catch':
            # The tiers telescope up, then the stamp: a sub boom and a tritone ring.
            rise = min(1.0, t / 0.45)
            phase += TAU * (40 + 260 * rise ** 2) / RATE
            s = 0.35 * math.sin(phase) * (t < 0.5) * min(1.0, t * 20)
            for k in range(12):
                s += 0.2 * click(t, 0.035 * k, 1200 + 70 * k, 90)
            s += 1.5 * thump(t, 0.5, 44, 7)
            s += 0.8 * low * math.exp(-max(0.0, t - 0.5) * 10) * (t > 0.5)
            s += 0.3 * bronze(t, 0.5, 220, 2.6) + 0.22 * bronze(t, 0.5, 311, 2.9)
        elif name == 'plumb_hum':
            # A pale drone with three orbits whooshing past at their own rates.
            s = 0.4 * math.sin(TAU * 165 * t) + 0.2 * math.sin(TAU * 247.5 * t)
            for rate, width in ((0.75, 0.6), (1.0, 0.5), (1.25, 0.4)):
                s += width * air * (0.5 + 0.5 * math.sin(TAU * rate * t)) ** 3
            s *= 0.8
        elif name == 'plumb_seal':
            # Three rings settle, high to low, and the point sets down.
            s = 0.0
            for k, (at, f) in enumerate(((0.0, 1318), (0.09, 1046), (0.18, 880))):
                s += (0.5 - 0.08 * k) * bronze(t, at, f, 9)
            s += 0.8 * thump(t, 0.3, 110, 22)
        elif name == 'plumb_catch':
            # Six faces creak open, the core swells, and the stamp.
            s = 0.0
            for k in range(6):
                s += 0.25 * click(t, 0.05 + 0.06 * k + rattle[k], 420 - 20 * k, 30)
            phase += TAU * (90 + 150 * min(1.0, x * 2)) / RATE
            s += 0.4 * math.sin(phase) * math.sin(math.pi * min(1.0, x * 1.6))
            s += 1.3 * thump(t, 0.5, 50, 8) + 0.3 * bronze(t, 0.5, 330, 3)
        elif name == 'roller_fall':
            # A hollow cage landing on a face: a thud and a rattle of struts.
            s = 1.2 * thump(t, 0.0, 62, 14) + 0.7 * low * math.exp(-t * 18)
            for k in range(5):
                s += 0.18 * click(t, 0.01 + rattle[k], 1900 + 300 * k, 60)
        elif name == 'roller_balance':
            # It tips onto one point, and a pure tone holds, as if nothing should.
            s = 0.6 * thump(t, 0.0, 80, 20)
            d = t - 0.12
            if d > 0:
                s += 0.45 * math.sin(TAU * 523.25 * d) * min(1.0, d * 8) * math.exp(-d * 1.2)
                s += 0.12 * math.sin(TAU * 1046.5 * d) * min(1.0, d * 8) * math.exp(-d * 2.0)
        elif name == 'roller_catch':
            # The halves split with a hiss and a clank, and the stamp.
            s = 0.3 * air * math.exp(-t * 5) * min(1.0, t * 60)
            s += 0.5 * click(t, 0.15, 650, 25) + 1.3 * thump(t, 0.4, 48, 8)
            s += 0.28 * bronze(t, 0.4, 262, 3.2)
        else:
            raise ValueError(name)
        samples.append(s)
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
        for index, (name, (duration, _peak, loop)) in enumerate(CUES.items()):
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
