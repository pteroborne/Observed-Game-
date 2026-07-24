#!/usr/bin/env python3
"""
SVG to JPG Converter for Documentation Evidence in Observed 2.
Renders SVG vector CAD blueprints into high-resolution JPG images using Pillow & ElementTree.
"""

import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

def parse_color(color_str, opacity=1.0):
    """Convert hex color strings (#0a121e, #00e5ff) to RGBA tuples."""
    if not color_str or color_str == "none":
        return None
    color_str = color_str.strip().lstrip("#")
    if len(color_str) == 6:
        r = int(color_str[0:2], 16)
        g = int(color_str[2:4], 16)
        b = int(color_str[4:6], 16)
        a = int(opacity * 255)
        return (r, g, b, a)
    return (0, 229, 255, int(opacity * 255))

def convert_svg_to_jpg(svg_path, jpg_path, quality=95):
    """Parse a CAD SVG blueprint and render a high-resolution JPG image."""
    svg_path = Path(svg_path)
    jpg_path = Path(jpg_path)
    
    if not svg_path.exists():
        raise FileNotFoundError(f"SVG file not found: {svg_path}")
        
    tree = ET.parse(svg_path)
    root = tree.getroot()
    
    width = int(root.attrib.get("width", 1600))
    height = int(root.attrib.get("height", 1200))
    
    # Create high-resolution RGBA canvas
    img = Image.new("RGBA", (width, height), (10, 18, 30, 255))
    draw = ImageDraw.Draw(img, "RGBA")
    
    # Process elements recursively
    def render_element(elem, offset_x=0.0, offset_y=0.0):
        tag = elem.tag.split("}")[-1]  # Strip namespace
        
        # Handle group transforms: transform="translate(x, y)"
        tx, ty = offset_x, offset_y
        transform = elem.attrib.get("transform", "")
        if "translate" in transform:
            try:
                coords = transform.split("translate(")[1].split(")")[0].split(",")
                tx += float(coords[0].strip())
                ty += float(coords[1].strip())
            except Exception:
                pass
                
        fill_str = elem.attrib.get("fill", "none")
        fill_opacity = float(elem.attrib.get("fill-opacity", 1.0))
        stroke_str = elem.attrib.get("stroke", "none")
        stroke_width = float(elem.attrib.get("stroke-width", 1.0))
        
        fill_color = parse_color(fill_str, fill_opacity)
        stroke_color = parse_color(stroke_str)
        
        if tag == "rect":
            x = float(elem.attrib.get("x", 0)) + tx
            y = float(elem.attrib.get("y", 0)) + ty
            w = float(elem.attrib.get("width", 0))
            h = float(elem.attrib.get("height", 0))
            box = [x, y, x + w, y + h]
            if fill_color:
                draw.rectangle(box, fill=fill_color)
            if stroke_color:
                draw.rectangle(box, outline=stroke_color, width=int(stroke_width))
                
        elif tag == "line":
            x1 = float(elem.attrib.get("x1", 0)) + tx
            y1 = float(elem.attrib.get("y1", 0)) + ty
            x2 = float(elem.attrib.get("x2", 0)) + tx
            y2 = float(elem.attrib.get("y2", 0)) + ty
            if stroke_color:
                draw.line([x1, y1, x2, y2], fill=stroke_color, width=int(stroke_width))
                
        elif tag == "polygon":
            pts_str = elem.attrib.get("points", "")
            if pts_str:
                raw_pts = pts_str.strip().split()
                poly_pts = []
                for pt in raw_pts:
                    coords = pt.split(",")
                    poly_pts.append((float(coords[0]) + tx, float(coords[1]) + ty))
                if len(poly_pts) >= 3:
                    if fill_color:
                        draw.polygon(poly_pts, fill=fill_color)
                    if stroke_color:
                        draw.polygon(poly_pts, outline=stroke_color)
                        
        elif tag == "circle":
            cx = float(elem.attrib.get("cx", 0)) + tx
            cy = float(elem.attrib.get("cy", 0)) + ty
            r = float(elem.attrib.get("r", 2.0))
            box = [cx - r, cy - r, cx + r, cy + r]
            if fill_color:
                draw.ellipse(box, fill=fill_color)
            if stroke_color:
                draw.ellipse(box, outline=stroke_color)
                
        elif tag == "text":
            x = float(elem.attrib.get("x", 0)) + tx
            y = float(elem.attrib.get("y", 0)) + ty
            text_content = elem.text or ""
            font_color = parse_color(fill_str) or (0, 229, 255, 255)
            font_size = int(elem.attrib.get("font-size", 12))
            
            try:
                font = ImageFont.truetype("arial.ttf", font_size)
            except Exception:
                font = ImageFont.load_default()
                
            draw.text((x, y - font_size * 0.8), text_content, fill=font_color, font=font)
            
        for child in elem:
            render_element(child, tx, ty)
            
    render_element(root)
    
    # Convert RGBA to RGB and save as JPG
    rgb_img = Image.new("RGB", (width, height), (10, 18, 30))
    rgb_img.paste(img, mask=img.split()[3])
    
    jpg_path.parent.mkdir(parents=True, exist_ok=True)
    rgb_img.save(jpg_path, "JPEG", quality=quality)
    print(f"Converted SVG -> JPG: {jpg_path}")

def main():
    if len(sys.argv) < 2:
        print("Usage: python convert_svg_to_jpg.py <input.svg> [output.jpg]")
        sys.exit(1)
        
    svg_path = Path(sys.argv[1])
    if len(sys.argv) >= 3:
        jpg_path = Path(sys.argv[2])
    else:
        jpg_path = svg_path.with_suffix(".jpg")
        
    convert_svg_to_jpg(svg_path, jpg_path)

if __name__ == "__main__":
    main()
