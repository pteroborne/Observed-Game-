"""Fit the CAD legend, normalize SVG whitespace and render with librsvg."""
from pathlib import Path
import subprocess
import sys
source = Path(sys.argv[1]).read_text()
source = source.replace('font-size="16" font-weight="bold" fill="#00e5ff">OBSERVED 2 CAD BLUEPRINT', 'font-size="13" font-weight="bold" fill="#00e5ff">OBSERVED 2 CAD BLUEPRINT')
source = source.replace('font-size="12" fill="#ffb703">ARCHETYPE', 'font-size="11" fill="#ffb703">ARCHETYPE')
output = Path(__file__).with_name("promenade-plan.svg")
output.write_text("\n".join(line.rstrip() for line in source.splitlines()) + "\n")
subprocess.run(["rsvg-convert", "-w", "1800", str(output), "-o", str(output.with_suffix('.png'))], check=True)
