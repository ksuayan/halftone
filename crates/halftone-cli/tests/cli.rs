use image::{DynamicImage, ImageBuffer, Rgb};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_halftone"))
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().unwrap()
}

fn photo(dir: &Path) -> PathBuf {
    let p = dir.join("photo.jpg");
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(240, 180, |x, y| {
        Rgb([((x + y) % 256) as u8, (x % 256) as u8, (y % 256) as u8])
    }))
    .save(&p)
    .unwrap();
    p
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn two_step_workflow_matches_convert() {
    let dir = tempfile::tempdir().unwrap();
    let img = photo(dir.path());
    let (cfg, svg, svg2, png) = (
        dir.path().join("c.json"),
        dir.path().join("a.svg"),
        dir.path().join("b.svg"),
        dir.path().join("p.png"),
    );
    let o = run(&["analyze", s(&img), "--paper", "a5", "-o", s(&cfg)]);
    assert!(o.status.success(), "{}", stderr(&o));
    let o = run(&["generate", s(&cfg), "-o", s(&svg), "--preview", s(&png)]);
    assert!(o.status.success(), "{}", stderr(&o));
    let o = run(&["convert", s(&img), "--paper", "a5", "-o", s(&svg2)]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(std::fs::read(&svg).unwrap(), std::fs::read(&svg2).unwrap());
    assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));
    let o = run(&["validate", s(&cfg), "--svg", s(&svg)]);
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn overrides_are_validated_like_json() {
    let dir = tempfile::tempdir().unwrap();
    let img = photo(dir.path());
    let cfg = dir.path().join("c.json");
    assert!(
        run(&["analyze", s(&img), "--paper", "a5", "-o", s(&cfg)])
            .status
            .success()
    );
    let out = dir.path().join("x.svg");
    let o = run(&["generate", s(&cfg), "-o", s(&out), "--max-coverage", "2"]);
    assert_eq!(o.status.code(), Some(4));
    assert!(stderr(&o).contains("tone.max_coverage"), "{}", stderr(&o));
    assert!(!out.exists(), "no output on failure");

    let saved = dir.path().join("eff.json");
    let o = run(&[
        "generate",
        s(&cfg),
        "-o",
        s(&out),
        "--set-angle",
        "-22.5",
        "--no-registration-marks",
        "--save-config",
        s(&saved),
    ]);
    // --no-registration-marks is an analyze flag, not a generate override.
    assert_eq!(o.status.code(), Some(2));
    let o = run(&[
        "generate",
        s(&cfg),
        "-o",
        s(&out),
        "--set-angle",
        "-22.5",
        "--registration-marks",
        "false",
        "--save-config",
        s(&saved),
    ]);
    assert!(o.status.success(), "{}", stderr(&o));
    let text = std::fs::read_to_string(&saved).unwrap();
    assert!(
        text.contains("\"angle_deg\": 67.5") && text.contains("\"registration_marks\": false"),
        "{text}"
    );
    assert!(!std::fs::read_to_string(&out).unwrap().contains("registration-marks"));
}

#[test]
fn exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let img = photo(dir.path());
    let o = run(&[
        "convert",
        s(&dir.path().join("missing.jpg")),
        "--paper",
        "a5",
        "-o",
        "x.svg",
    ]);
    assert_eq!(o.status.code(), Some(3), "{}", stderr(&o));
    let o = run(&["convert", s(&img), "--paper", "b5", "-o", s(&dir.path().join("x.svg"))]);
    assert_eq!(o.status.code(), Some(4), "{}", stderr(&o));
    assert!(stderr(&o).contains("output.paper"));
    let o = run(&[
        "convert",
        s(&img),
        "--paper",
        "a5",
        "-o",
        s(&dir.path().join("no/such/dir/x.svg")),
    ]);
    assert_eq!(o.status.code(), Some(6), "{}", stderr(&o));
    let o = run(&["convert", s(&img), "-o", "x.svg"]);
    assert_eq!(
        o.status.code(),
        Some(4),
        "missing paper is a config error: {}",
        stderr(&o)
    );
    let tiny = dir.path().join("tiny.svg");
    let o = run(&[
        "convert",
        s(&img),
        "--width",
        "40",
        "--height",
        "40",
        "--units",
        "mm",
        "--lpi",
        "5",
        "-o",
        s(&tiny),
    ]);
    assert_eq!(o.status.code(), Some(5), "page too small for any cell: {}", stderr(&o));
    assert!(!tiny.exists());
}

#[test]
fn schema_and_version() {
    let o = run(&["schema"]);
    assert!(o.status.success());
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("\"schema_version\"") && text.contains("\"tolerance_um\""));
    let o = run(&["version"]);
    assert!(String::from_utf8(o.stdout).unwrap().starts_with("halftone 0."));
}
