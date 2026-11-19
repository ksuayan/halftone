# Configuration

`halftone analyze` writes a JSON configuration; `halftone generate` reads it.
It is the complete description of one job: edit it, keep it, re-run it.

- Strict: unknown fields are rejected, and errors name the field
  (`screen.lpi: invalid type: string "22", expected f64`).
- Lengths are millimetres and say so (`_mm`); geometry tolerance is in µm.
- `recommendations` records what analysis proposed; the other sections hold
  the values actually used; `user_set` lists settings you supplied.
- `layout` is derived. `generate` recomputes it, so editing it has no effect.
- `source.path` is relative to the config file's folder when the image is
  inside it, otherwise absolute. `source.blake3` lets `generate` warn when the
  image has changed since analysis.

JSON Schema: [config.schema.json](config.schema.json) (regenerate with `halftone schema`).

## Example

A3 paper, 110-mesh screen, landscape photo (paper turned to match):

```json
{
  "schema_version": 1,
  "app_version": "0.1.0",
  "source": {
    "path": "photo.jpg",
    "blake3": "698578bbd33ff827ad2c769a9ae8d343a454796ff41b41ba97b992d7c7eca882",
    "width_px": 6000,
    "height_px": 4000,
    "orientation": "NoTransforms",
    "pixel_format": "Rgb8",
    "color": "untagged: sRGB assumed"
  },
  "output": {
    "paper": "a3",
    "paper_width_mm": 420.0,
    "paper_height_mm": 297.0,
    "printer_margin_mm": 6.35,
    "fit": "contain",
    "ink": "#000000",
    "registration_marks": true,
    "display_units": "in"
  },
  "screen": {
    "grid": "cartesian",
    "shape": "circle",
    "lpi": 22.0,
    "angle_deg": 22.5,
    "mesh_count": 110.0,
    "mesh_ratio": 5.0
  },
  "tone": {
    "invert": false,
    "black_point": 0.0,
    "white_point": 1.0,
    "gamma": 1.0,
    "min_dot_mm": 0.13,
    "max_coverage": 1.0,
    "analysis_background": "#FFFFFF"
  },
  "geometry": { "tolerance_um": 10.0, "px_per_cell": 2.0 },
  "layout": {
    "page_width_mm": 407.3,
    "page_height_mm": 284.3,
    "image": { "x_mm": 12.0, "y_mm": 14.383, "width_mm": 383.3, "height_mm": 255.533 },
    "pitch_mm": 1.154545,
    "cells": 72698
  },
  "recommendations": {
    "profile": "screen-print",
    "lpi": 22.0,
    "lpi_source": "mesh",
    "lpi_range": [22.0, 27.5],
    "notes": ["LPI is mesh_count / mesh_ratio, a rule of thumb; confirm with a test print on your screen, ink and substrate."]
  },
  "user_set": ["output.paper"]
}
```

## Fields

| Field | Meaning | Valid range |
| --- | --- | --- |
| `output.paper_width_mm`, `paper_height_mm` | Paper size | 20–2000 mm |
| `output.printer_margin_mm` | Unprintable border per side; the SVG page is the paper minus this on every side | 0–50 mm |
| `output.ink` | Fill colour `#RRGGBB` (warning unless `#000000`) | |
| `output.registration_marks` | Four circle-and-crosshair targets in a 12 mm band | |
| `screen.lpi` | Lines per inch along the lattice axes; pitch = 25.4 / lpi mm | 5–150 (warning above mesh/4) |
| `screen.angle_deg` | Screen rotation, normalized to [0, 90) | |
| `screen.mesh_count`, `mesh_ratio` | Mesh-based recommendation: lpi = mesh / ratio | 40–500, 3–8 |
| `tone.black_point`, `white_point` | Linear luminance treated as solid black / white | 0–1, black < white |
| `tone.gamma` | Normalized luminance raised to 1/gamma; >1 lightens midtones | 0.1–10 |
| `tone.invert` | Light areas get large dots | |
| `tone.min_dot_mm` | Smaller dots are not drawn | 0–5 mm, below the largest dot |
| `tone.max_coverage` | Cap on ink coverage, e.g. 0.9 keeps shadows open | 0.05–1 |
| `geometry.tolerance_um` | Maximum radial error of polygonal dots | 1–100 µm |
| `geometry.px_per_cell` | Working resolution per screen cell | 1–8 |

## Tone

Target coverage is `1 − Y`, where `Y` is linear luminance after the black/white
points and gamma: a printed patch then reflects the same light as the source,
before dot gain. Dot diameter comes from the exact coverage of a periodic
lattice of equal circles, so overlapping shadow dots still produce the
requested coverage. Print compensation (dot gain) is not applied; use
`gamma` and `max_coverage` until a calibration curve exists.
