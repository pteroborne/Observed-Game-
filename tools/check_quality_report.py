#!/usr/bin/env python3
"""Compare a screenshot-free production route report with the accepted budgets.

This checks timing evidence, not hardware identity or human acceptance. Run the
benchmark separately from visual capture; record hardware/build alongside it.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path

BUDGETS = {"desktop": (16667, 25000, 33333), "deck": (33333, 50000, 66667)}


def assess(report, preset):
    if not isinstance(report, dict):
        raise ValueError("expected a timing report object")
    failures = []
    count = lambda value: isinstance(value, int) and not isinstance(value, bool) and value >= 0
    schema = report.get("schema_version", 0)
    if not count(schema) or schema < 5:
        failures.append("schema 5 or later is required; older reports undercount mutations")
    if report.get("rules") != "ascent" or report.get("grid") != [24, 17, 8]:
        failures.append("requires the production eight-floor Ascent workload")
    if report.get("vsync_uncapped") is not True:
        failures.append("requires uncapped benchmark evidence")
    runs = report.get("route_ticks_runs", [])
    if not isinstance(runs, list) or not runs or not count(runs[0]) or runs[0] < 7200:
        failures.append("requires at least 7,200 recorded simulation ticks")
    warm = report.get("warmed_frames", {})
    if not isinstance(warm, dict):
        raise ValueError("expected warmed frame statistics")
    if not count(warm.get("samples", 0)) or warm.get("samples", 0) < 2000:
        failures.append("requires at least 2,000 warmed frame samples")
    metrics = {}
    for name, limit in zip(("p95", "p99", "maximum"), BUDGETS[preset]):
        value = warm.get(f"{name}_microseconds")
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
            failures.append(f"{name}: missing or invalid measurement")
            continue
        metrics[name] = {"measured_ms": round(value / 1000, 3), "budget_ms": round(limit / 1000, 3)}
        if value > limit:
            failures.append(f"{name}: {value / 1000:.3f} ms exceeds {limit / 1000:.3f} ms")
    return {"preset_budget": preset, "passed": not failures, "metrics": metrics, "failures": failures,
            "acceptance_boundary": "Timing comparison only. Hardware identity, capture method, human play and physical LAN/device gates require separate evidence."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--preset", choices=BUDGETS, default="desktop")
    args = parser.parse_args()
    try:
        raw = args.report.read_bytes()
        report = json.loads(raw)
        result = assess(report, args.preset)
    except (OSError, ValueError, TypeError, KeyError) as error:
        parser.exit(2, f"invalid timing report: {error}\n")
    result["report_sha256"] = hashlib.sha256(raw).hexdigest()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result["passed"] else 1)


if __name__ == "__main__":
    main()
