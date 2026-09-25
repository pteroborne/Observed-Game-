#!/usr/bin/env python3
"""Compile guardian_form_lab's film frames into MP4s, with their sound.

The capture writes frames only (screenshots carry no audio). This rebuilds each
film's soundtrack from the same timeline the capture used, and the sounds from
tools/generate_guardian_audio.py, then muxes the two with ffmpeg:

  seen:  hunting 0-3.0 s, then frozen by sight until 5.5 s
  catch: hunting 0-1.5 s, then the catch until 3.3 s

These mirror SEEN and CATCH in labs/guardian_form_lab/src/capture.rs, and the cues
mirror labs/guardian_form_lab/src/sound.rs; change them together.

  python3 tools/mux_guardian_films.py <capture dir> [--out <dir>]

Write to a local directory and copy the results into the repository: on the ntfs3
mount the repository lives on, ffmpeg writing one of these MP4s in place stalled
indefinitely (2026-09-25), past even `timeout`, while the same command into /tmp
finished at once.
"""
import argparse
from pathlib import Path
import subprocess

FPS = 30
TIMELINES = {'seen': (3.0, 5.5, 'freeze'), 'catch': (1.5, 3.3, 'catch')}
VOICES = {
    'tumbler_4': {'hum': 'tumbler_hum', 'step': None, 'freeze': 'tumbler_latch',
                  'catch': 'tumbler_catch'},
    'plumb': {'hum': 'plumb_hum', 'step': None, 'freeze': 'plumb_seal',
              'catch': 'plumb_catch'},
    'roller': {'hum': None, 'step': 'roller_fall', 'freeze': 'roller_balance',
               'catch': 'roller_catch'},
}
ROLLER_STEP = 1.0


def mux(frames: Path, out: Path, form: str, film: str, sounds: Path):
    change, end, cue = TIMELINES[film]
    voice = VOICES[form]
    inputs = ['-framerate', str(FPS), '-i', str(frames / 'f_%04d.png')]
    chains = []
    labels = []

    def add(name, start, length=None):
        index = 1 + len(labels)
        inputs.extend(['-i', str(sounds / f'{name}.ogg')])
        chain = f'[{index}:a]'
        if length is not None:
            # A hum stops at the change of state, with the briefest fade, not a click.
            chain += f'atrim=0:{length:.3f},afade=t=out:st={max(0.0, length - 0.06):.3f}:d=0.06,'
        chain += f'adelay={round(start * 1000)}:all=1[a{index}]'
        chains.append(chain)
        labels.append(f'[a{index}]')

    if voice['hum']:
        # The hum loops in play; no film hums longer than its 4 s, so one pass is enough.
        assert change <= 4.0
        add(voice['hum'], 0.0, length=change)
    if voice['step']:
        k = 1
        while k * ROLLER_STEP <= change:
            add(voice['step'], k * ROLLER_STEP)
            k += 1
    add(voice[cue], change)
    graph = ';'.join(chains) + ';' + ''.join(labels) + \
        f'amix=inputs={len(labels)}:normalize=0,apad,atrim=0:{end:.3f}[mix]'
    # -nostdin: run from anything but a terminal, ffmpeg otherwise waits on stdin.
    subprocess.run(['ffmpeg', '-nostdin', '-loglevel', 'error', '-y', *inputs,
                    '-filter_complex', graph, '-map', '0:v', '-map', '[mix]',
                    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '20',
                    '-c:a', 'aac', '-b:a', '128k', '-shortest',
                    '-movflags', '+faststart', str(out)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    out = args.out or args.capture
    sounds = Path(__file__).resolve().parents[1] / 'assets/sounds/guardian'
    for frames in sorted(args.capture.glob('film_*_*')):
        if not frames.is_dir():
            continue
        name = frames.name[len('film_'):]
        form, film = name.rsplit('_', 1)
        target = out / f'{frames.name}.mp4'
        mux(frames, target, form, film, sounds)
        print(target)


if __name__ == '__main__':
    main()
