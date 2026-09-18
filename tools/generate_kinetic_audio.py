#!/usr/bin/env python3
"""Original kinetic-lab sound palette. Stdlib synthesis + ffmpeg Vorbis encoder.

No recordings or borrowed assets. Fixed per-cue seeds, 48 kHz mono, 6 dB peak
headroom, short fade edges. In-game gains provide the mix; this script never
rewrites the shared game sound palette. --preview writes a labeled audition reel.
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
CUES = {
    'push': (0.46, 0.49), 'pull': (0.62, 0.43),
    'miss': (0.14, 0.16), 'empty': (0.26, 0.19),
    'impact': (0.35, 0.31), 'step': (0.16, 0.21),
    'guardian_step': (0.19, 0.22), 'guardian_voice': (0.66, 0.22),
    'void': (0.95, 0.29), 'warning': (1.9, 0.26),
    'retract': (0.95, 0.37), 'power_on': (0.75, 0.24),
    'power_off': (0.70, 0.23), 'wave': (0.85, 0.30),
    'clear': (1.25, 0.30), 'capture': (0.80, 0.29),
    'recharge': (0.60, 0.23), 'charge_tick': (0.17, 0.09),
    'land': (0.26, 0.27),
}


def synth(name, duration, peak, seed):
    rng = random.Random(seed)
    samples = []
    low = 0.0
    phase = 0.0
    for i in range(round(RATE * duration)):
        t = i / RATE
        x = t / duration
        noise = rng.uniform(-1, 1)
        low += 0.055 * (noise - low)
        air = noise - low
        def tone(freq):
            return math.sin(TAU * freq * t)
        def decay(rate):
            return math.exp(-t * rate)
        if name == 'push':
            phase += TAU * (51 + 155 * math.exp(-t * 25)) / RATE
            s = math.sin(phase) * decay(10) + 0.7 * low * decay(13) + 0.12 * air * decay(35)
            s += 0.10 * tone(830) * decay(48)
        elif name == 'pull':
            phase += TAU * (65 + 260 * x * x) / RATE
            swell = math.sin(math.pi * min(1, x * 1.1)) ** 0.7
            s = 0.36 * math.sin(phase) * swell + 1.4 * low * swell + 0.045 * air * swell
            s += 0.18 * tone(125) * decay(23)
        elif name in ('step', 'guardian_step', 'land', 'impact'):
            f = {'step': 105, 'guardian_step': 235, 'land': 66, 'impact': 83}[name]
            s = 0.65 * tone(f) * decay(30) + 1.5 * low * decay(21) + 0.12 * air * decay(65)
            if name == 'guardian_step':
                s += 0.22 * tone(613) * decay(44) + 0.1 * tone(997) * decay(60)
            if name == 'impact':
                s += 0.28 * tone(187) * decay(19) + 0.13 * tone(431) * decay(32)
        elif name == 'guardian_voice':
            phase += TAU * (142 + 19 * math.sin(TAU * 3.5 * t)) / RATE
            s = (math.sin(phase) + 0.19 * math.sin(phase * 2.71)) * math.sin(math.pi * x) ** 2
            s += low * 0.5 * math.sin(math.pi * x)
        elif name in ('miss', 'empty'):
            s = (tone(170 if name == 'empty' else 330) * 0.4 + low) * decay(24)
            if name == 'empty' and t > 0.105:
                s += 0.25 * tone(125) * math.exp(-(t - 0.105) * 30)
        elif name == 'warning':
            local = t % 0.48
            s = (tone(440) * 0.35 + tone(660) * 0.15) * math.exp(-local * 18) * min(1, local * 400)
        elif name in ('retract', 'void', 'power_off', 'capture'):
            phase += TAU * (45 + 130 * (1-x) ** 2) / RATE
            s = math.sin(phase) * math.sin(math.pi * x) * 0.45 + low * 1.7 * math.sin(math.pi * x)
            if name == 'retract':
                s += air * 0.16 * (1-x) + tone(72) * decay(8)
            elif name == 'capture':
                s += tone(111) * 0.25 * decay(3) + tone(149) * 0.15 * decay(4)
        else:
            notes = {'wave': [165, 220, 196], 'clear': [262, 330, 392],
                     'recharge': [392, 523, 659], 'power_on': [98, 147, 196],
                     'charge_tick': [550]}[name]
            s = 0.
            for j, f in enumerate(notes):
                nt = t - j * duration * 0.19
                if nt >= 0:
                    s += (math.sin(TAU*f*nt) + 0.18*math.sin(TAU*f*2.01*nt)) * math.exp(-nt * 9) * min(1, nt * 220)
            s += low * decay(12) * 0.2
        fade = min(1., t / 0.005, (duration-t) / 0.025)
        samples.append(s * fade)
    # Remove DC; retain intentional differences in cue loudness.
    mean = sum(samples) / len(samples)
    samples = [s-mean for s in samples]
    scale = peak / max(abs(s) for s in samples)
    return [s*scale for s in samples]


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
    parser.add_argument('--out', type=Path, default=Path(__file__).resolve().parents[1] / 'assets/sounds/kinetic')
    parser.add_argument('--preview', type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    report = {}
    reel = []
    labels = []
    with tempfile.TemporaryDirectory() as temp:
        for index, (name, (duration, peak)) in enumerate(CUES.items()):
            samples = synth(name, duration, peak, 9100 + index)
            assert all(math.isfinite(s) for s in samples)
            assert max(abs(s) for s in samples) <= 0.5
            wav = Path(temp) / (name + '.wav')
            write_wav(wav, samples)
            subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', str(wav), '-c:a', 'libvorbis', '-q:a', '5', str(args.out / (name + '.ogg'))], check=True)
            report[name] = {'duration': duration, 'peak_dbfs': round(20*math.log10(peak), 2), 'rms_dbfs': round(20*math.log10(math.sqrt(sum(s*s for s in samples)/len(samples))), 2)}
            start = len(reel) / RATE
            reel.extend(samples)
            reel.extend([0.] * round(RATE * 0.4))
            labels.append((start, len(reel) / RATE, name.replace('_', ' ').upper()))
        if args.preview:
            args.preview.parent.mkdir(parents=True, exist_ok=True)
            write_wav(args.preview, reel)
            def timestamp(seconds):
                milliseconds = round(seconds * 1000)
                return f"{milliseconds // 3600000:02}:{milliseconds // 60000 % 60:02}:{milliseconds // 1000 % 60:02},{milliseconds % 1000:03}"
            args.preview.with_suffix('.srt').write_text('\n'.join(
                f"{i}\n{timestamp(start)} --> {timestamp(end)}\n{name}\n"
                for i, (start, end, name) in enumerate(labels, 1)
            ))
    (args.out / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Generated {len(report)} original cues in {args.out}')

if __name__ == '__main__':
    main()
