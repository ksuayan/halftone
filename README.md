# Halftone

Convert photographs into single-ink halftone SVG positives for screen printing
and linocut. Pure Rust; macOS first, Linux and Windows supported.

- Regular screen of variable-size circles; dark tones → larger dots.
- Tone-accurate: dot sizes account for overlap between neighbouring dots.
- One compound SVG path in physical units, transparent background, plus
  optional industry-standard registration targets.
- Two-stage workflow with a reusable, editable JSON configuration.

## Build

```bash
cargo build --release
# binary: target/release/halftone
```

## Use

```bash
# One step: photo -> SVG (and a PNG preview)
halftone convert portrait.jpg --paper a3 --mesh 110 -o portrait.svg --preview portrait.png

# Two steps: analyze, review/edit the JSON, then generate
halftone analyze portrait.jpg --paper 9x12 --mesh 110 -o portrait.halftone.json
halftone generate portrait.halftone.json -o portrait.svg --set-angle 15 --max-coverage 0.9

# Check a config and an SVG made from it
halftone validate portrait.halftone.json --svg portrait.svg
```

Common options:

| Option | Meaning |
| --- | --- |
| `--paper NAME` | letter, legal, tabloid, 9x12, a5, a4, a3 |
| `--width W --height H --units in\|cm\|mm` | custom paper size |
| `--orientation auto\|portrait\|landscape` | `auto` matches the photo |
| `--mesh N` / `--lpi N` | screen mesh count (LPI = mesh / 5) or explicit LPI |
| `--angle DEG` | screen angle (default 22.5°) |
| `--printer-margin-mm MM` | printer's unprintable border (default 6.35 mm) |
| `--registration-marks` | add registration targets (off by default) |
| `--invert` | light areas get large dots |
| `--min-dot-mm`, `--max-coverage`, `--gamma`, `--black-point`, `--white-point` | tone overrides at generate time |
| `--preview FILE.png`, `--save-config FILE.json`, `-v` | extra outputs and diagnostics |

### Printing

The SVG page is your paper minus the printer margin on each side, so printing
with **Fit to Page** scales by about 1.0. With `--registration-marks`, measure
the distance between the targets on the film to confirm the scale. Print in pure black
(`#000000`); laser printers screen any other colour into their own pattern.

Run `halftone --help` and `halftone <command> --help` for every option.

### Batch test runs

```bash
scripts/test-run.sh                          # prototypes/photos -> prototypes/output/test-run (letter, 110 mesh)
scripts/test-run.sh -p a3 --lpi 25 --angle 15 --gamma 1.5
PAPER=9x12 MAX_COVERAGE=0.9 scripts/test-run.sh --invert --registration-marks
```

For each photo it writes the SVG, a PNG preview, the effective config and a
verbose log, then prints a summary. Page, screen, tone and geometry options can
be set as flags or environment variables (`scripts/test-run.sh --help`).

## Documentation

- [docs/Halftone.md](docs/Halftone.md) — specification
- [docs/Halftone Algos.md](docs/Halftone%20Algos.md) — algorithm research
- [docs/CLI.md](docs/CLI.md) — every command, option and accepted value
- [docs/architecture.md](docs/architecture.md) — crates and pipeline
- [docs/configuration.md](docs/configuration.md) — the JSON configuration ([schema](docs/config.schema.json))
- [docs/Phase0-Report.md](docs/Phase0-Report.md) — prototype measurements behind the design
- [docs/TODO.md](docs/TODO.md) — plan and decision log

## License

MIT
