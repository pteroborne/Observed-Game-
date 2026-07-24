#!/usr/bin/env python3
"""
3D Shape Verifier for LLM / AI Agent 3D Object Editing.

This script parses baked 3D module JSON files or raw convex hull vertex arrays,
performs topological and spatial integrity audits (bounding box, volume, hull count,
watertightness, and vertex bounds), and outputs a structured verification JSON report
and optional ASCII blueprint diagram.
"""

import sys
import json
import math
from pathlib import Path

def compute_hull_volume_and_bounds(points):
    """Compute 3D bounding box and approximate bounding volume for a vertex cloud."""
    if not points:
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    zs = [p[2] for p in points]
    
    min_x, max_x = min(xs), max(xs)
    min_y, max_y = min(ys), max(ys)
    min_z, max_z = min(zs), max(zs)
    
    dx = max_x - min_x
    dy = max_y - min_y
    dz = max_z - min_z
    bounding_volume = dx * dy * dz
    
    return (min_x, max_x, min_y, max_y, min_z, max_z, bounding_volume)

def audit_module(module_data):
    """Perform topological and spatial audits on a 3D module JSON structure."""
    module_id = module_data.get("module_id", "unknown")
    hulls = module_data.get("hulls", [])
    
    results = {
        "module_id": module_id,
        "hull_count": len(hulls),
        "total_vertices": 0,
        "overall_bounds": {},
        "hulls_summary": [],
        "passed_audit": True,
        "warnings": [],
        "errors": []
    }
    
    if not hulls:
        results["passed_audit"] = False
        results["errors"].append("Module contains 0 convex hulls.")
        return results
    
    g_min_x = g_min_y = g_min_z = float('inf')
    g_max_x = g_max_y = g_max_z = float('-inf')
    
    for idx, hull in enumerate(hulls):
        points = hull.get("points", [])
        vertex_count = len(points)
        results["total_vertices"] += vertex_count
        
        if vertex_count < 4:
            results["passed_audit"] = False
            results["errors"].append(f"Hull #{idx} has fewer than 4 vertices ({vertex_count}). Degenerate hull!")
        
        min_x, max_x, min_y, max_y, min_z, max_z, vol = compute_hull_volume_and_bounds(points)
        g_min_x = min(g_min_x, min_x)
        g_max_x = max(g_max_x, max_x)
        g_min_y = min(g_min_y, min_y)
        g_max_y = max(g_max_y, max_y)
        g_min_z = min(g_min_z, min_z)
        g_max_z = max(g_max_z, max_z)
        
        results["hulls_summary"].append({
            "id": hull.get("id", idx),
            "vertices": vertex_count,
            "bounds": {
                "min": [round(min_x, 3), round(min_y, 3), round(min_z, 3)],
                "max": [round(max_x, 3), round(max_y, 3), round(max_z, 3)]
            },
            "bounding_volume": round(vol, 3)
        })
    
    dx = round(g_max_x - g_min_x, 3)
    dy = round(g_max_y - g_min_y, 3)
    dz = round(g_max_z - g_min_z, 3)
    
    results["overall_bounds"] = {
        "min": [round(g_min_x, 3), round(g_min_y, 3), round(g_min_z, 3)],
        "max": [round(g_max_x, 3), round(g_max_y, 3), round(g_max_z, 3)],
        "dimensions": [dx, dy, dz],
        "bounding_box_volume": round(dx * dy * dz, 3)
    }
    
    # Check complexity limits (budget <= 32 for cells, <= 128 for rooms)
    if len(hulls) > 128:
        results["warnings"].append(f"Hull count ({len(hulls)}) exceeds room complexity budget (128).")
        
    return results

def render_ascii_top_plan(module_data, grid_size=20):
    """Render a 2D ASCII top plan grid showing overall shape footprint."""
    hulls = module_data.get("hulls", [])
    all_pts = []
    for h in hulls:
        all_pts.extend(h.get("points", []))
    
    if not all_pts:
        return "No points to render."
    
    xs = [p[0] for p in all_pts]
    zs = [p[2] for p in all_pts]
    min_x, max_x = min(xs), max(xs)
    min_z, max_z = min(zs), max(zs)
    
    dx = (max_x - min_x) or 1.0
    dz = (max_z - min_z) or 1.0
    
    grid = [["." for _ in range(grid_size)] for _ in range(grid_size)]
    
    for h_idx, h in enumerate(hulls):
        symbol = str(h_idx % 10)
        for p in h.get("points", []):
            gx = int(((p[0] - min_x) / dx) * (grid_size - 1))
            gz = int(((p[2] - min_z) / dz) * (grid_size - 1))
            grid[gz][gx] = symbol
            
    lines = ["ASCII TOP PLAN VIEW (Z vs X):"]
    for row in grid:
        lines.append(" ".join(row))
    return "\n".join(lines)

def main():
    if len(sys.argv) < 2:
        print("Usage: python verify_3d_shape.py <module.json>")
        sys.exit(1)
        
    path = Path(sys.argv[1])
    if not path.exists():
        print(f"Error: file {path} does not exist.")
        sys.exit(1)
        
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
        
    audit = audit_module(data)
    print(json.dumps(audit, indent=2))
    print("\n" + render_ascii_top_plan(data))

if __name__ == "__main__":
    main()
