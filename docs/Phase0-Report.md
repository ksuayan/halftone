# Halftone — Phase 0 Report

**Date:** 2026-11-18
**Scope:** Experiments A–D from [TODO.md](TODO.md) Phase 0, plus the
Experiment E test-chart generator. Experiment E's physical print runs are
still outstanding.
**Code:** [`prototypes/`](../prototypes) (throwaway; its own workspace root).
**Machine:** Intel i9-9900K (8 cores / 16 threads), 72 GB RAM, macOS, Rust 1.97.1.
This machine is not Apple Silicon. Timings are indicative; memory numbers are
judged against the 2 GB budget for an 8 GB Apple machine.

Reproduce from `prototypes/` with `cargo build --release`, then:

| Experiment | Command |
| --- | --- |
| A — color pipeline | `exp_a correctness <dir>`, `exp_a gen-large <dir>`, `exp_a pipeline <file> 297 420 45 4`, `exp_a downscale-error` |
| B + D — lattice and geometry | `exp_d <9x12-22\|9x12-45\|a3-22\|a3-45>[-dark] <check\|poly-st\|poly-mt\|tiled\|hybrid\|hybrid-poly> [out.svg]`, `exp_d arc-check` |
| C — coverage | `exp_c` |
| E — print chart | `exp_e <letter\|tabloid\|a3> <out.svg> [preview.png]` |

Measure peak memory with `/usr/bin/time -l` ("peak memory footprint").

---

## Summary of decisions

| # | Decision | Evidence |
| --- | --- | --- |
| D4 | Luminance from the image's own ICC primaries via `moxcms` (pure Rust), no gamut clipping; Y weights normalized so white = 1; untagged or gray images are treated as sRGB | §A.2 |
| D5 | Composite alpha over white in linear light (default); no other modes needed for v1 | §A.3 |
| D6 | `i_overlay` 9.2 (pure Rust). `clipper2` not needed | §D |
| D7 / D14 | Circles as area-preserving polygons; path written as line segments. Exact arcs rejected: renderers flatten them inaccurately | §D.4 |
| D8 | Radial tolerance 10 µm (≈ ¼ device pixel at 600 dpi), pending visual check in Exp E | §D.3 |
| D9 / D27 | Integer geometry, 1 unit = 1 µm. Output is identical between runs and between 1 and N threads on this machine. Cross-platform byte-identity not yet tested | §D.2 |
| D10 / D24 | Union only the marks that overlap a neighbor; emit isolated marks directly as polygons. `i_overlay` multithreading on. Tiled union rejected | §D.1 |
| D12 / D22 / D26 | Downscale in linear light to **2 px per cell**, box filter, then center bilinear sample. That downscale *is* the area-average and anti-moiré filter; no separate area sampler or blur needed | §A.4 |
| D13 | Closed-form periodic coverage plus exact inverse by bisection (no lookup table needed) | §C |
| D3 | LPI = frequency along the lattice axes, pitch = 25.4 / LPI mm; phase origin = image center; cells indexed (i, j) in lattice space, so row/column counts are not meaningful for rotated grids. The config should record the index ranges and cell count | §B |
| D32 | Max input ≈ 150 MP (≈ 10–12 bytes per input pixel at peak) to fit 2 GB | §A.5 |

---

## A. Image and color pipeline

### A.1 Decoding

`image` 0.25.10 (features `jpeg`, `png`, `tiff`, `rayon`) decodes 8- and
16-bit JPEG/PNG/TIFF and exposes `icc_profile()` and `orientation()` on the
decoder. `image` already depends on `moxcms`, but only uses it for
enumerated (CICP) color spaces, so arbitrary embedded ICC profiles need
`moxcms` called directly.

### A.2 Luminance and ICC

- **Approach:** for a matrix-shaper RGB profile, transform to a copy of the
  same profile with linear (gamma 1.0) curves. That linearizes in the image's
  own primaries. Then Y = the Y row of that profile's colorant matrix.
- **Why not convert to sRGB first:** out-of-gamut colors clip. A Display P3
  red would become Y = 0.2224 instead of its true 0.2411.
- **Pitfall found:** the copied profile must have its `cicp` field cleared.
  Otherwise `moxcms` applies the CICP transfer function and ignores the
  replacement curves, so mid-gray stays unchanged at 0.502.
- **Scanner profiles:** device profiles have colorants that don't sum to 1
  (white measured 0.35–1.10). Weights are normalized so (1, 1, 1) → Y = 1
  (relative colorimetric).
- **Checks, all passing:**
  - Untagged 8-bit ramp vs the sRGB transfer function: error 9×10⁻⁸.
  - 16-bit TIFF ramp: error 1×10⁻⁷.
  - Untagged, sRGB-tagged and P3-tagged mid-gray all give 0.2159.
  - P3 primaries match the profile exactly.
- **Real-world profiles:** tested against every ICC profile installed with
  macOS, about 270 RGB profiles.
  - Standard spaces (sRGB, Display P3, Adobe RGB, ROMM, Rec. 709/2020,
    eciRGB, ACEScg) and 256 SilverFast scanner profiles use the
    matrix-shaper path.
  - Two LUT-based profiles use the fallback path (linear sRGB, clips).
  - One (`WebSafeColors.icc`) is unusable. The product should **warn and
    assume sRGB**.
- **Limitations to document:** gray ICC profiles are ignored (sRGB curve
  assumed). CMYK JPEGs are out of scope.

### A.3 Alpha and orientation

- Alpha is composited over white in linear light: error 4×10⁻⁸.
- Orientation is applied to the 4-byte luminance plane rather than the RGB
  decode buffer, so rotating never duplicates the large buffer. All 8 EXIF
  orientations match `image::apply_orientation` exactly.

### A.4 Downscale = sampling filter

The test image is a synthetic 5460 × 7720 photo-like plane with smooth
tones, fine texture and hard edges, on A3 at 22.5°. The reference is the
true area average of each cell (16 × 16 supersamples). Mean absolute
luminance error:

| Method | 22 LPI | 45 LPI |
| --- | --- | --- |
| Center sample of the full-resolution image | 0.0250 | 0.0197 |
| 1 px/cell, box | 0.0153 | 0.0083 |
| **2 px/cell, box** | **0.0127** | **0.0069** |
| 3 px/cell, box | 0.0105 | 0.0104 |
| 4 px/cell, box | 0.0128 | 0.0132 |
| 6 px/cell, box | 0.0175 | 0.0172 |

Lanczos3 is within ±0.001 of box. Downscaling to 2–3 px per cell halves the
error of full-resolution sampling, because the downscale acts as the area
average and prefilter. Choice: **2 px/cell, box** (no ringing, cheapest).
Downscaling takes 10–60 ms.

### A.5 Time and memory (stages run in sequence; decode buffer dropped after luminance)

| Input | Decode | Luminance | Downscale | Total | Peak footprint |
| --- | --- | --- | --- | --- | --- |
| 60 MP 16-bit RGB TIFF | 383 ms | 95 ms | 75 ms | 0.55 s | 721 MB |
| 24 MP 8-bit JPEG | 121 ms | 52 ms | 44 ms | 0.22 s | 219 MB |

Peak memory is the decoded buffer plus the `f32` luminance plane. Decoding
TIFFs in strips is **not needed** for the 2 GB budget. A safety limit around
150 MP is sufficient (D32).

---

## B. Lattice

- Rotated Cartesian lattice in mm, with the phase origin at the image
  center.
- The whole-cells-only rule keeps a cell when a disk of radius p/√2 (the
  largest possible mark) around its center fits inside the image. Result:
  0 marks cross the edge at either test size.
- Inverse transform round-trip error: 2×10⁻¹³ cells.
- Cell counts: 9 × 12 at 22 LPI = 51,622; A3 at 45 LPI = 389,734.
- Rows are generated in parallel and collected in order, so output is
  deterministic.
- At 22.5° the whole-cell edge is a staircase (visible in the chart
  preview). That is expected under this policy; edge trimming remains a
  future option.

## C. Coverage → diameter

- A square lattice point's Voronoi cell is its own square cell, so periodic
  union coverage = one disk clipped to its cell. That gives a closed form for
  every d ∈ [0, √2·p].
- Validated against a 9-disk brute force (2000² samples): maximum error
  2×10⁻⁵ (sampling noise). The inverse is monotonic over 100,001 samples,
  with round-trip error 3×10⁻¹⁵.
- The isolated-circle formula saturates near 91% actual coverage. A request
  for 99% would print 90.6%. The overlap correction matters for everything
  above 78.5%.
- Cost: 400k inversions take 0.38 s single-threaded (trivial with Rayon). No
  lookup table needed.
- Tone convention: target coverage = 1 − Y_linear (reflectance model, no
  dot gain). sRGB mid-gray 128 → 78% coverage. Dot-gain compensation stays a
  later, optional curve.

## D. Geometry

Synthetic photo at 2 px/cell, 22.5°, minimum dot 0.13 mm, area-preserving
polygons, integer µm. The `-dark` variant scales luminance × 0.25 (75–100%
coverage, nearly all marks overlap), which is the worst case.

### D.1 Union strategies

Tolerance 5 µm unless noted.

| Case | Method | Geometry time | Peak footprint | SVG |
| --- | --- | --- | --- | --- |
| 9×12 @ 22 | all-polygon, 1 thread | 0.80 s | 224 MB | 8.1 MB |
| 9×12 @ 22 | all-polygon, `i_overlay` threads | 0.68 s | 228 MB | 8.1 MB |
| 9×12 @ 22 | tiled (32² cells, tree merge) | 1.90 s | 236 MB | 8.1 MB |
| A3 @ 45 | all-polygon, 1 thread | 4.9 s | 746 MB | 39.7 MB |
| A3 @ 45 | all-polygon, threads | 3.9 s | 880 MB | 39.7 MB |
| A3 @ 45 | tiled | 10.1 s | 563 MB | 39.7 MB |
| A3 @ 45 dark | all-polygon, 1 thread | 8.4 s | 1.30 GB | 39.0 MB |
| A3 @ 45 dark | all-polygon, threads | 6.2 s | 1.36 GB | 39.0 MB |

- **Tiled union is rejected.** Its merge tree redoes most of the work: 2–3×
  slower and 3× the CPU time.
- **`i_overlay`'s built-in threading** gives 1.2–1.4×. Keep it on.

### D.2 Determinism

On this machine:

- Two multithreaded unions produce identical output.
- Multithreaded and single-threaded output is identical.
- Tiled and single-union areas agree to within 7×10⁻⁵ mm².

Cross-platform identity (Apple Silicon vs x86, Windows) is still untested.
Integer µm geometry limits any difference to the one rounding of each
vertex.

### D.3 Tolerance

A3 @ 45 dark, union-overlapping-only method:

| Tolerance | Time | Peak | Output vertices | SVG | Ink |
| --- | --- | --- | --- | --- | --- |
| 2.5 µm | 6.8 s | 1.56 GB | 5.80 M | 43.4 MB | 85.726% |
| 5 µm | 5.6 s | 1.40 GB | 4.60 M | 36.4 MB | 85.726% |
| **10 µm** | **4.5 s** | **0.94 GB** | 3.55 M | 30.4 MB | 85.721% |
| 20 µm | 3.5 s | 0.76 GB | 2.87 M | 25.8 MB | 85.680% |

Area-preserving polygons keep tone unchanged down to 10 µm. 10 µm is about
¼ of a 600 dpi device pixel. It is the tentative default, to be confirmed by
eye in Exp E.

### D.4 Arcs vs polygons

The tested idea was to write isolated marks as exact SVG arcs: smaller
files, and union only where marks overlap. Rendered ink, in percent of area,
for 1600 isolated dots of one radius:

| Radius | Exact | Arcs @ 600 dpi | Polygons @ 600 dpi | Arcs @ 4800 dpi | Polygons @ 4800 dpi |
| --- | --- | --- | --- | --- | --- |
| 0.065 mm | 0.922 | 0.900 | 0.923 | 0.916 | 0.920 |
| 0.100 mm | 2.182 | 2.131 | 2.193 | 2.169 | 2.188 |
| 0.250 mm | 13.635 | 13.580 | 13.697 | 13.621 | 13.675 |
| 0.400 mm | 34.907 | 34.570 | 34.987 | 34.890 | 34.934 |

At printer resolution, `resvg` flattens arcs so that dots lose 1–2.4% of
their area. Other renderers (Affinity, printer drivers) will flatten arcs
differently, so arc output gives up control of tone. Polygons render within
±0.3% of exact. **Decision:** polygons only.

The speed benefit survives without arcs: union only the overlapping marks
and write isolated marks directly as polygons (`hybrid-poly`):

| Case (10 µm) | All-polygon union | Union overlapping only | Rendered ink, both |
| --- | --- | --- | --- |
| 9×12 @ 22 | 0.48 s | **0.08 s** | 43.829% |
| A3 @ 45 | 2.76 s | **0.52 s** | 44.194% |
| A3 @ 45 dark | 4.55 s | 4.49 s | 85.743% |

The output is identical in vertex count and rendered ink. The worst case
(dark A3 at 45 LPI) stays at about 4.5 s and under 1 GB. That leaves the
whole pipeline about 100× under the 10-minute ceiling and about 2× under the
2 GB budget.

**Correction (2026-11-18, found during Phase 4):** the prototype's
overlap test compared *circle* radii. Area-preserving polygons put their
vertices up to about 5% outside the circle (8-sided small dots), so two
marks that nearly touch could produce overlapping polygons when emitted
separately. The `geo` validity test caught this. The product compares
polygon vertex radii instead. Timings and tone results above are
unaffected.

### D.5 Renderer checks

`resvg` renders every output. Rendered ink matches geometric ink within
0.02 points at 600 dpi, and holes and fill rule render correctly. Rendering
an A3 page at 600 dpi takes 3–5 s, which is fine for tests and for the PNG
preview.

**Lesson for `--preview`:** flatten the PNG onto white. A transparent
background shows as solid black in common viewers.

---

## E. Print test chart (generator ready; printing outstanding)

`exp_e` writes Letter, Tabloid and A3 charts with:

- Tonal wedges, 0–100% in 5% steps, at 22, 24 and 27.5 LPI.
- Shadow ramps, 80–100% in 2% steps.
- A minimum-dot ramp of isolated dots 1–8 device pixels wide (42–339 µm).
- Angle variants at 0°, 15°, 22.5°, 30° and 45°, each at 50% and 90%.
- 100 mm and 4 in scale bars.
- Four registration targets (circle plus crosshair, 0.5 pt), with the
  expected target spacing at 100% printed on the chart.

Labels are SVG `<text>` (chart only).

Steps for Kyo:

1. Print the chart for your printer's paper with Fit to Page onto
   transparency film.
2. Measure the scale bar and the target spacing. This gives the actual Fit
   to Page scale and the printer's real margin, which settles
   `printer_margin` (D29).
3. Expose and print on the 110-mesh screen.
4. Record:
   - The smallest dot that survives on film and on the print (minimum dot;
     currently assumed 0.13 mm).
   - The coverage where shadows fill in (D25).
   - The best angle (D1) and LPI (D2).
   - Any visible polygon faceting (D8).

---

## Still open after Phase 0

- **Exp E (needs printing):** D1, D2, D8 (visual), D25, the minimum-dot
  value, and `printer_margin`.
- **Affinity Designer and Inkscape import:** load and edit performance of
  roughly 30 MB / 3.5 M-vertex SVGs (A3 at 45 LPI). Test files can be
  produced with `exp_d a3-45 hybrid-poly out.svg`.
- **D27:** run the same job on Apple Silicon and compare output bytes.
- **D21:** final crate graph, decided at the start of Phase 1.
