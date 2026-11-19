# Halftone CLI reference

`halftone` converts photographs into single-ink halftone SVG positives. This page
lists every command, option and accepted value. `halftone --help` and
`halftone <command> --help` show the same options in short form.

```text
halftone [--threads N] [-v] <COMMAND>

  analyze   photo  -> JSON configuration (no SVG)
  generate  config -> SVG (with optional overrides)
  convert   photo  -> SVG in one step (analyze + generate)
  validate  check a configuration, and optionally an SVG made from it
  schema    print the configuration JSON Schema
  version   print the version
```

---

## Global options

Accepted by every command, before or after the command name.

| Option | Value | Default | Meaning |
| --- | --- | --- | --- |
| `--threads N` | positive integer | all cores | Worker threads. Output is identical for any thread count. |
| `-v`, `--verbose` | flag | off | After `generate`/`convert`: effective settings, user-set fields, geometry statistics and stage timings (on stderr). |
| `-h`, `--help` | flag | | Help for the command. |
| `-V`, `--version` | flag | | Print the version. |

---

## `halftone analyze`

Inspects the image header (it does not decode pixels), recommends screen
settings and writes a reusable JSON configuration.

```bash
halftone analyze <INPUT> (--paper NAME | --width W --height H) -o <CONFIG.json> [options]
```

| Argument / option | Value | Default | Meaning |
| --- | --- | --- | --- |
| `<INPUT>` | path | required | Source photo: JPEG, PNG or TIFF (8- or 16-bit; RGB, gray, with or without alpha). |
| `-o`, `--output` | path | required | Configuration file to write. |
| `--paper` | `letter`, `legal`, `tabloid`, `9x12`, `a5`, `a4`, `a3` (any case) | | Named paper size. Give either this or `--width`/`--height`. |
| `--width`, `--height` | number in `--units` | | Custom paper size. Both are required together; not allowed with `--paper`. |
| `--units` | `in`, `cm`, `mm` (also `inch`, `inches`) | `in` | Units for `--width`/`--height` and for sizes in reports. |
| `--orientation` | `auto`, `portrait`, `landscape` | `auto` | Paper orientation. `auto` turns the paper to match the photo. |
| `--profile` | `screen-print`, `linocut` | `screen-print` | Supplies defaults (see [Profiles](#profiles)). |
| `--mesh` | 40 to 500 | profile (110 for screen-print) | Screen mesh count, threads per inch. LPI is recommended as mesh ÷ 5. |
| `--lpi` | 5 to 150 | from mesh or profile | Screen frequency in lines per inch. Takes precedence over the mesh recommendation. |
| `--angle` | degrees, any value | profile (22.5) | Screen angle. Normalized to 0–90° (a square screen repeats every 90°): `-22.5` becomes `67.5`. Negative values are accepted. |
| `--ink` | `#RRGGBB` | `#000000` | Ink colour. Anything other than pure black gives a warning, because laser printers screen it into their own pattern. |
| `--printer-margin-mm` | 0 to 50 (always mm) | 6.35 | The printer's non-printable border per side. The SVG page is the paper minus this on every side, so Fit to Page prints at about 100%. |
| `--registration-marks` | flag | off | Add four registration targets in a 12 mm band around the image. |
| `--invert` | flag | off | Light areas get large dots instead of dark areas. |

### Paper sizes

Portrait dimensions; `--orientation auto` turns them to match the photo.

| Name | Inches | Millimetres |
| --- | --- | --- |
| `letter` | 8.5 × 11 | 215.9 × 279.4 |
| `legal` | 8.5 × 14 | 215.9 × 355.6 |
| `tabloid` | 11 × 17 | 279.4 × 431.8 |
| `9x12` | 9 × 12 | 228.6 × 304.8 |
| `a5` | 5.83 × 8.27 | 148 × 210 |
| `a4` | 8.27 × 11.69 | 210 × 297 |
| `a3` | 11.69 × 16.54 | 297 × 420 |

Custom sizes accept 20–2000 mm per side.

### LPI selection

1. `--lpi` if given (recorded as user-set);
2. otherwise mesh ÷ 5, using `--mesh` or the profile's mesh count;
3. otherwise the profile's LPI (linocut: 12).

The advisory range mesh ÷ 5 to mesh ÷ 4 is recorded in the configuration.
An LPI finer than mesh ÷ 4 gives a warning, because dots may not hold on the
screen.

---

## `halftone generate`

Reads a configuration, applies any overrides, and writes a validated SVG.

```bash
halftone generate <CONFIG.json> -o <OUT.svg> [options]
```

| Argument / option | Value | Default | Meaning |
| --- | --- | --- | --- |
| `<CONFIG>` | path | required | Configuration written by `analyze` (or edited by hand). |
| `-o`, `--output` | path | required | SVG file to write. |
| `--preview` | path | none | Also write a PNG preview of the page on a white background. |
| `--preview-dpi` | number | 150 | Preview resolution. Previews over 400 million pixels are refused. |
| `--save-config` | path | none | Write the effective configuration, including overrides and the refreshed layout. |

### Overrides

Overrides change the configuration for this run only (use `--save-config` to
keep them). They are checked with the same rules as the JSON file, and each one
is added to `user_set` in the configuration.

| Option | Value | Configuration field | Meaning |
| --- | --- | --- | --- |
| `--set-lpi` | 5 to 150 | `screen.lpi` | Screen frequency. |
| `--set-angle` | degrees | `screen.angle_deg` | Screen angle, normalized to 0–90°. |
| `--set-ink` | `#RRGGBB` | `output.ink` | Ink colour. |
| `--min-dot-mm` | 0 to 5, and smaller than the largest dot (√2 × pitch) | `tone.min_dot_mm` | Dots smaller than this are not drawn. |
| `--max-coverage` | 0.05 to 1 | `tone.max_coverage` | Cap on ink coverage; for example 0.9 keeps shadows from closing up. |
| `--gamma` | 0.1 to 10 | `tone.gamma` | Midtone gamma. Values above 1 lighten midtones. |
| `--black-point` | 0 to 1, below the white point | `tone.black_point` | Linear luminance at or below this prints as solid black. |
| `--white-point` | 0 to 1, above the black point | `tone.white_point` | Linear luminance at or above this prints no ink. |
| `--tolerance-um` | 1 to 100 | `geometry.tolerance_um` | Maximum radial error of the polygonal dots, in µm (default 10). |
| `--set-registration-marks` | `true`, `false` | `output.registration_marks` | Turn the registration targets on or off. |

Settings that have no override flag (paper, printer margin, invert, mesh) can
be changed by editing the JSON; see [configuration.md](configuration.md).

---

## `halftone convert`

Runs `analyze` and then `generate` in one step, through the same code. It
accepts every `analyze` option and every `generate` option. Its `-o` names the
**SVG**.

```bash
halftone convert <INPUT> (--paper NAME | --width W --height H) -o <OUT.svg> [analyze options] [generate options]
```

- `--save-config` writes the configuration used. Without it, no JSON file is written.
- When both are given, generate overrides win over analyze options. For
  example, `--registration-marks --set-registration-marks false` leaves the marks out.
- Output is byte-identical to running `analyze` then `generate` with the same settings.

---

## `halftone validate`

```bash
halftone validate <CONFIG.json> [--svg <FILE.svg>]
```

| Argument / option | Meaning |
| --- | --- |
| `<CONFIG>` | Configuration to check: schema version, every range rule, colours, consistency (for example black point below white point). Warnings are printed; errors fail. |
| `--svg` | Also check an SVG against that configuration: well-formed XML, page size in mm, µm `viewBox`, exactly one halftone path in the ink colour with an explicit fill rule, registration-marks path present only if enabled, no other elements, and every point inside the page. |

To check an SVG made with `generate` overrides, validate it against the
configuration saved with `--save-config`.

---

## `halftone schema` and `halftone version`

- `halftone schema` prints the JSON Schema of the configuration file. The repository copy is [config.schema.json](config.schema.json).
- `halftone version` prints `halftone <version>`.

---

## Profiles

| Setting | `screen-print` | `linocut` (provisional) |
| --- | --- | --- |
| Mesh count | 110 | none |
| LPI | mesh ÷ 5 = 22 | 12 |
| Angle | 22.5° | 45° |
| Minimum dot | 0.13 mm (about 3 printer dots at 600 dpi) | 0.5 mm |
| Max coverage | 1.0 | 0.95 |
| Printer margin | 6.35 mm | 6.35 mm |
| Polygon tolerance | 10 µm | 10 µm |
| Working resolution | 2 px per screen cell | 2 px per screen cell |

All values are starting points until confirmed by test prints (TODO
decisions D1, D2, D16).

---

## Fixed behaviour

These aren't options in v1:

- **Grid and dot shape:** square (Cartesian) grid, round dots.
- **Fit:** the whole photo fits inside the image area (contain), centred, aspect ratio kept.
- **Edges:** only whole cells are drawn, so no dot is cut off at the edge.
- **Alpha:** transparent pixels are treated as white.
- **Registration targets** (when enabled): a circle (3 mm radius) with a 10 mm crosshair at the middle of each side, drawn with a 0.5 pt line in the ink colour.
- **Colour:** embedded ICC profiles are honoured. Untagged images are treated as sRGB; an unusable profile gives a warning and falls back to sRGB.

### Safety limits

| Limit | Value | Error |
| --- | --- | --- |
| Input image | 150 megapixels | exit 3 |
| Screen cells | 2,000,000 | exit 5 |
| Points to union (overlapping dots) | 6,000,000 | exit 5 |

---

## Outputs

- Files are written to a temporary file and then renamed, so a failed run
  never leaves a partial or stale-looking file.
- SVG: page size in mm (paper minus printer margins), `viewBox` in
  micrometres, one `<path id="halftone">`, an optional
  `<path id="registration-marks">`, and nothing else.
- Normal output, such as `wrote …` lines and the analysis summary, goes to
  stdout. Warnings, errors and `--verbose` details go to stderr.

## Exit codes

| Code | Meaning | Examples |
| --- | --- | --- |
| 0 | Success | |
| 2 | Invalid command line | unknown option, missing `-o`, `--paper` with `--width` |
| 3 | Input problem | photo missing, unreadable or corrupt; over the pixel limit |
| 4 | Configuration problem | value out of range, unknown paper, missing paper size, malformed or unknown JSON field |
| 5 | Processing problem | too many cells or dots for the limits; no room for the image on the page; image area too small to hold a single whole screen cell |
| 6 | Output problem | cannot write the SVG, preview or configuration |
| 70 | Internal error | generated SVG failed its own validation (please report) |

## Examples

```bash
# A3, 110-mesh screen, with preview
halftone convert photo.jpg --paper a3 --mesh 110 -o photo.svg --preview photo.png

# Custom 30 × 40 cm, explicit 25 LPI at 15°, with registration marks
halftone convert photo.jpg --width 30 --height 40 --units cm --lpi 25 --angle 15 --registration-marks -o photo.svg

# Two steps: analyze, then try lighter tones without editing the JSON
halftone analyze photo.jpg --paper 9x12 -o photo.halftone.json
halftone generate photo.halftone.json -o photo.svg --gamma 1.5 --max-coverage 0.9 --save-config photo.effective.json
halftone validate photo.effective.json --svg photo.svg

# Linocut profile with inverted tone (light areas get large dots)
halftone convert photo.jpg --paper a4 --profile linocut --invert -o lino.svg
```
