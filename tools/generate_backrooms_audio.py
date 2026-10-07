#!/usr/bin/env python3
"""Original physical building loops; deterministic synthesis, no music bed."""
import pathlib
import subprocess
import tempfile
import wave
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[1] / "assets/sounds/backrooms"
RATE = 22050


def generate():
    ROOT.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(20261006)
    for name, seconds in [("hvac", 12), ("fluorescent", 12), ("creak", 3)]:
        count = RATE * seconds
        time = np.arange(count) / RATE
        noise = rng.normal(size=count)
        spectrum = np.fft.rfft(noise)
        frequency = np.fft.rfftfreq(count, 1 / RATE)
        spectrum *= np.exp(-frequency / 800) * (1 - np.exp(-frequency / 70))
        air = np.fft.irfft(spectrum, n=count)
        air /= max(np.max(np.abs(air)), 1e-9)
        if name == "hvac":
            sound = air * .38 * (1 + .12 * np.sin(2 * np.pi * time / seconds))
            sound += .025 * np.sin(2 * np.pi * 60 * time)
        elif name == "fluorescent":
            sound = .09 * np.sin(2 * np.pi * 120 * time)
            sound += .025 * np.sin(2 * np.pi * 240 * time) + .10 * air
        else:
            envelope = np.sin(np.pi * time / seconds) ** 2
            sound = envelope * (.30 * air + .08 * np.sin(2 * np.pi * (190 * time - 15 * time ** 2)))
        # Short symmetric loop joins; all files peak below -6 dBFS.
        fade = min(int(RATE * .03), count // 2)
        sound[:fade] *= np.linspace(0, 1, fade)
        sound[-fade:] *= np.linspace(1, 0, fade)
        with tempfile.TemporaryDirectory() as tmp:
            source = pathlib.Path(tmp) / "source.wav"
            with wave.open(str(source), "wb") as output:
                output.setparams((1, 2, RATE, count, "NONE", "not compressed"))
                output.writeframes((np.clip(sound, -.5, .5) * 32767).astype("<i2").tobytes())
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(source), "-c:a", "libvorbis", "-q:a", "5", str(ROOT / (name + ".ogg"))], check=True)
        print(name, "peak", round(float(np.max(np.abs(sound))), 4))


if __name__ == "__main__":
    generate()
