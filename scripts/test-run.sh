#!/usr/bin/env bash
# Convert every photo in a folder to a halftone SVG (plus PNG preview, effective
# config and a verbose log per photo), then print a summary.
#
# Usage: scripts/test-run.sh [options] [-- EXTRA HALFTONE CONVERT ARGS]
#
# Every option can also be set with the environment variable in brackets,
# e.g. `PAPER=a3 LPI=24 scripts/test-run.sh`. Command-line flags win.
# Options left empty are not passed, so the app's own default applies
# (see docs/CLI.md).
#
# Files
#   -i, --input DIR            photo folder               [INPUT]   prototypes/photos
#   -o, --output DIR           output folder              [OUTPUT]  prototypes/output/test-run
#       --no-build             use the existing release binary [NO_BUILD=1]
#
# Page
#   -p, --paper NAME           letter legal tabloid 9x12 a5 a4 a3 [PAPER] letter
#       --width W --height H   custom size (replaces --paper)     [WIDTH] [HEIGHT]
#       --units U              in, cm, mm (for --width/--height)  [UNITS]
#       --orientation O        auto, portrait, landscape          [ORIENTATION]
#       --printer-margin-mm M  non-printable border per side      [PRINTER_MARGIN_MM]
#       --registration-marks   add registration targets           [MARKS=1]  off
#
# Screen
#       --profile P            screen-print, linocut              [PROFILE]
#   -m, --mesh N               screen mesh count                  [MESH]    110
#       --lpi N                screen frequency (overrides mesh)  [LPI]
#       --angle DEG            screen angle                       [ANGLE]
#       --ink #RRGGBB          ink colour                         [INK]
#
# Tone and geometry
#       --invert               light areas get large dots         [INVERT=1]
#       --min-dot-mm MM        smallest dot drawn                 [MIN_DOT_MM]
#       --max-coverage C       ink coverage cap, 0..1             [MAX_COVERAGE]
#       --gamma G              midtone gamma (>1 lightens)        [GAMMA]
#       --black-point B        0..1                               [BLACK_POINT]
#       --white-point W        0..1                               [WHITE_POINT]
#       --tolerance-um U       polygon tolerance in µm            [TOLERANCE_UM]
#
# Run
#       --preview-dpi DPI      PNG preview resolution             [PREVIEW_DPI]
#       --threads N            worker threads                     [THREADS]
#   -h, --help                 show this help
#
# Exit status: 0 if every photo converted, 1 if any failed, 2 on bad usage.

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Defaults (environment overrides, then flags).
INPUT="${INPUT:-$ROOT/prototypes/photos}"
OUTPUT="${OUTPUT:-$ROOT/prototypes/output/test-run}"
NO_BUILD="${NO_BUILD:-0}"
PAPER="${PAPER:-letter}"
WIDTH="${WIDTH:-}"
HEIGHT="${HEIGHT:-}"
UNITS="${UNITS:-}"
ORIENTATION="${ORIENTATION:-}"
PRINTER_MARGIN_MM="${PRINTER_MARGIN_MM:-}"
MARKS="${MARKS:-0}"
PROFILE="${PROFILE:-}"
MESH="${MESH:-110}"
LPI="${LPI:-}"
ANGLE="${ANGLE:-}"
INK="${INK:-}"
INVERT="${INVERT:-0}"
MIN_DOT_MM="${MIN_DOT_MM:-}"
MAX_COVERAGE="${MAX_COVERAGE:-}"
GAMMA="${GAMMA:-}"
BLACK_POINT="${BLACK_POINT:-}"
WHITE_POINT="${WHITE_POINT:-}"
TOLERANCE_UM="${TOLERANCE_UM:-}"
PREVIEW_DPI="${PREVIEW_DPI:-}"
THREADS="${THREADS:-}"
EXTRA=()

usage() { sed -n '2,46p' "$0" | sed 's/^# \{0,1\}//'; }
die() { echo "test-run: $*" >&2; exit 2; }
# value OPTION NEXT: require a value after OPTION.
value() { [ $# -ge 2 ] && [ -n "$2" ] || die "$1 needs a value (see --help)"; echo "$2"; }

while [ $# -gt 0 ]; do
    case "$1" in
        -i|--input) INPUT=$(value "$@") || exit 2; shift 2 ;;
        -o|--output) OUTPUT=$(value "$@") || exit 2; shift 2 ;;
        --no-build) NO_BUILD=1; shift ;;
        -p|--paper) PAPER=$(value "$@") || exit 2; shift 2 ;;
        --width) WIDTH=$(value "$@") || exit 2; shift 2 ;;
        --height) HEIGHT=$(value "$@") || exit 2; shift 2 ;;
        --units) UNITS=$(value "$@") || exit 2; shift 2 ;;
        --orientation) ORIENTATION=$(value "$@") || exit 2; shift 2 ;;
        --printer-margin-mm) PRINTER_MARGIN_MM=$(value "$@") || exit 2; shift 2 ;;
        --registration-marks) MARKS=1; shift ;;
        --profile) PROFILE=$(value "$@") || exit 2; shift 2 ;;
        -m|--mesh) MESH=$(value "$@") || exit 2; shift 2 ;;
        --lpi) LPI=$(value "$@") || exit 2; shift 2 ;;
        --angle) ANGLE=$(value "$@") || exit 2; shift 2 ;;
        --ink) INK=$(value "$@") || exit 2; shift 2 ;;
        --invert) INVERT=1; shift ;;
        --min-dot-mm) MIN_DOT_MM=$(value "$@") || exit 2; shift 2 ;;
        --max-coverage) MAX_COVERAGE=$(value "$@") || exit 2; shift 2 ;;
        --gamma) GAMMA=$(value "$@") || exit 2; shift 2 ;;
        --black-point) BLACK_POINT=$(value "$@") || exit 2; shift 2 ;;
        --white-point) WHITE_POINT=$(value "$@") || exit 2; shift 2 ;;
        --tolerance-um) TOLERANCE_UM=$(value "$@") || exit 2; shift 2 ;;
        --preview-dpi) PREVIEW_DPI=$(value "$@") || exit 2; shift 2 ;;
        --threads) THREADS=$(value "$@") || exit 2; shift 2 ;;
        --) shift; EXTRA=("$@"); break ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1 (see --help)" ;;
    esac
done

[ -d "$INPUT" ] || die "input folder not found: $INPUT"
if [ -n "$WIDTH" ] || [ -n "$HEIGHT" ]; then
    [ -n "$WIDTH" ] && [ -n "$HEIGHT" ] || die "--width and --height go together"
    PAPER=""   # a custom size replaces the named paper
fi

# Build `halftone convert` arguments from the settings that are set.
ARGS=()
add() { [ -n "$2" ] && ARGS+=("$1" "$2"); return 0; }
add --paper "$PAPER"
add --width "$WIDTH"
add --height "$HEIGHT"
add --units "$UNITS"
add --orientation "$ORIENTATION"
add --printer-margin-mm "$PRINTER_MARGIN_MM"
add --profile "$PROFILE"
add --mesh "$MESH"
add --lpi "$LPI"
# --angle=VALUE so negative angles are not taken for options.
[ -n "$ANGLE" ] && ARGS+=("--angle=$ANGLE")
add --ink "$INK"
add --min-dot-mm "$MIN_DOT_MM"
add --max-coverage "$MAX_COVERAGE"
add --gamma "$GAMMA"
add --black-point "$BLACK_POINT"
add --white-point "$WHITE_POINT"
add --tolerance-um "$TOLERANCE_UM"
add --preview-dpi "$PREVIEW_DPI"
add --threads "$THREADS"
[ "$MARKS" = 1 ] && ARGS+=(--registration-marks)
[ "$INVERT" = 1 ] && ARGS+=(--invert)
# ${X[@]+...} keeps bash 3.2 happy with `set -u` when an array is empty.
ARGS+=(${EXTRA[@]+"${EXTRA[@]}"})

BIN="$ROOT/target/release/halftone"
if [ "$NO_BUILD" != 1 ]; then
    echo "building release binary..."
    (cd "$ROOT" && cargo build --release -q -p halftone-cli) || exit 1
fi
[ -x "$BIN" ] || die "binary not found: $BIN (run without --no-build)"

mkdir -p "$OUTPUT"
echo "settings: ${ARGS[*]}"
echo "output:   $OUTPUT"
echo

ok=0
failed=0
# NUL-separated so file names with spaces work; sorted for a stable order.
while IFS= read -r -d '' photo; do
    name="$(basename "$photo")"
    stem="${name%.*}"
    log="$OUTPUT/$stem.log"
    start=$SECONDS
    if "$BIN" convert "$photo" \
        -o "$OUTPUT/$stem.svg" \
        --preview "$OUTPUT/$stem.png" \
        --save-config "$OUTPUT/$stem.halftone.json" \
        -v "${ARGS[@]}" >"$log" 2>&1; then
        status="ok"
        ok=$((ok + 1))
        size=$(du -h "$OUTPUT/$stem.svg" | cut -f1)
        marks=$(sed -n 's/.*, \([0-9]*\) marks).*/\1/p' "$log" | head -1)
        warnings=$(grep -c '^warning' "$log")
        detail="$size, ${marks:-?} marks"
        [ "$warnings" -gt 0 ] && detail="$detail, $warnings warning(s) in $stem.log"
    else
        status="FAILED"
        failed=$((failed + 1))
        detail="$(grep -m1 '^error' "$log" || tail -1 "$log")"
    fi
    printf '%-7s %-34s %4ss  %s\n' "$status" "$name" "$((SECONDS - start))" "$detail"
done < <(find "$INPUT" -maxdepth 1 -type f \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.tif' -o -iname '*.tiff' \) -print0 | sort -z)

total=$((ok + failed))
echo
echo "$ok of $total converted; output in $OUTPUT"
[ "$total" -eq 0 ] && echo "no JPEG, PNG or TIFF files found in $INPUT" >&2
[ "$failed" -eq 0 ] && [ "$total" -gt 0 ]
