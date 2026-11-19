use halftone::config_io::{from_json, to_json};
use halftone::{AnalyzeRequest, Error, GenerateOptions, Limits, Orientation, PaperChoice, Profile, analyze, generate};
use halftone_types::Units;
use halftone_types::geometry::area_mm2;
use image::{DynamicImage, ImageBuffer, Rgb};
use std::path::{Path, PathBuf};

fn write_image(dir: &Path, name: &str, w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> PathBuf {
    let path = dir.join(name);
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, h, |x, y| Rgb([f(x, y); 3])))
        .save(&path)
        .unwrap();
    path
}

fn request(input: PathBuf, dir: &Path, paper: &str) -> AnalyzeRequest {
    AnalyzeRequest {
        input,
        config_dir: dir.to_path_buf(),
        paper: PaperChoice::Named(paper.into()),
        orientation: Orientation::Auto,
        display_units: Units::Mm,
        profile: Profile::ScreenPrint,
        mesh_count: None,
        lpi: None,
        angle_deg: None,
        ink: None,
        printer_margin_mm: None,
        registration_marks: None,
        invert: None,
    }
}

#[test]
fn analyze_recommends_from_mesh_and_records_user_choices() {
    let dir = tempfile::tempdir().unwrap();
    let img = write_image(dir.path(), "p.png", 300, 400, |x, _| x as u8);
    let (c, issues) = analyze(&request(img.clone(), dir.path(), "a5"), &Limits::default()).unwrap();
    assert!(issues.is_empty());
    assert_eq!(c.screen.lpi, 22.0);
    assert_eq!(c.recommendations.lpi_range, Some([22.0, 27.5]));
    assert_eq!(c.source.path, "p.png", "stored relative to the config directory");
    assert!(c.layout.cells > 0);
    assert_eq!(c.user_set, vec!["output.paper"]);
    assert!(!c.output.registration_marks, "registration marks are off by default");

    let mut req = request(img, dir.path(), "a5");
    req.lpi = Some(30.0);
    let (c, issues) = analyze(&req, &Limits::default()).unwrap();
    assert!(c.user_set.contains(&"screen.lpi".to_string()));
    assert!(
        issues.iter().any(|i| i.field == "screen.lpi"),
        "30 lpi is finer than the 110-mesh advisory range"
    );
}

#[test]
fn orientation_follows_the_photo() {
    let dir = tempfile::tempdir().unwrap();
    let wide = write_image(dir.path(), "w.png", 400, 200, |_, _| 128);
    let (c, _) = analyze(&request(wide.clone(), dir.path(), "a4"), &Limits::default()).unwrap();
    assert!(c.output.paper_width_mm > c.output.paper_height_mm);
    let mut req = request(wide, dir.path(), "a4");
    req.orientation = Orientation::Portrait;
    let (c, _) = analyze(&req, &Limits::default()).unwrap();
    assert!(c.output.paper_width_mm < c.output.paper_height_mm);
}

#[test]
fn uniform_gray_prints_its_target_coverage() {
    let dir = tempfile::tempdir().unwrap();
    // sRGB 128 -> linear Y 0.2159 -> target coverage 0.784.
    let img = write_image(dir.path(), "g.png", 200, 300, |_, _| 128);
    let mut req = request(img, dir.path(), "a5");
    req.registration_marks = Some(true);
    let (c, _) = analyze(&req, &Limits::default()).unwrap();
    let g = generate(&c, dir.path(), &Limits::default(), &GenerateOptions::default()).unwrap();
    let ink = area_mm2(&g.artwork);
    let cell_area = g.config.layout.cells as f64 * g.config.layout.pitch_mm.powi(2);
    let coverage = ink / cell_area;
    assert!((coverage - 0.784).abs() < 0.02, "coverage {coverage}");
    assert_eq!(g.page.targets.len(), 4);
}

#[test]
fn generation_is_deterministic_and_thread_independent() {
    let dir = tempfile::tempdir().unwrap();
    let img = write_image(dir.path(), "r.png", 240, 320, |x, y| ((x * 3 + y * 5) % 256) as u8);
    let (c, _) = analyze(&request(img, dir.path(), "a5"), &Limits::default()).unwrap();
    let a = generate(&c, dir.path(), &Limits::default(), &GenerateOptions::default()).unwrap();
    let b = generate(
        &c,
        dir.path(),
        &Limits::default(),
        &GenerateOptions { multithread: false },
    )
    .unwrap();
    assert_eq!(a.svg, b.svg);
}

#[test]
fn config_round_trips_and_rejects_bad_input() {
    let dir = tempfile::tempdir().unwrap();
    let img = write_image(dir.path(), "p.png", 100, 100, |_, _| 200);
    let (c, _) = analyze(&request(img, dir.path(), "a5"), &Limits::default()).unwrap();
    let json = to_json(&c);
    assert_eq!(from_json(&json, Path::new("c.json")).unwrap(), c);

    let unknown = json.replacen("\"lpi\":", "\"lines\": 1, \"lpi\":", 1);
    let err = from_json(&unknown, Path::new("c.json")).unwrap_err().to_string();
    assert!(err.contains("screen") && err.contains("lines"), "{err}");

    let mut bad = c.clone();
    bad.screen.lpi = f64::NAN;
    bad.tone.black_point = 0.9;
    bad.tone.white_point = 0.1;
    match generate(&bad, dir.path(), &Limits::default(), &GenerateOptions::default()) {
        Err(Error::Config(issues)) => {
            let fields: Vec<_> = issues.iter().map(|i| i.field.as_str()).collect();
            assert!(
                fields.contains(&"screen.lpi") && fields.contains(&"tone.black_point"),
                "{fields:?}"
            );
        }
        other => panic!("expected config error, got {:?}", other.err()),
    }
}

#[test]
fn changed_source_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let img = write_image(dir.path(), "p.png", 100, 120, |_, _| 90);
    let (c, _) = analyze(&request(img, dir.path(), "a5"), &Limits::default()).unwrap();
    write_image(dir.path(), "p.png", 100, 120, |_, _| 91);
    let g = generate(&c, dir.path(), &Limits::default(), &GenerateOptions::default()).unwrap();
    assert!(g.warnings.iter().any(|w| w.field == "source.blake3"));
}

#[test]
fn limits_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let img = write_image(dir.path(), "p.png", 100, 100, |_, _| 100);
    let tight = Limits {
        max_cells: 50,
        ..Limits::default()
    };
    assert!(matches!(
        analyze(&request(img.clone(), dir.path(), "a3"), &tight),
        Err(Error::Screen(_))
    ));
    let tiny = Limits {
        max_input_pixels: 99,
        ..Limits::default()
    };
    assert!(matches!(
        analyze(&request(img, dir.path(), "a3"), &tiny),
        Err(Error::Image(_))
    ));
}

#[test]
fn full_size_dots_stay_on_the_page_without_marks() {
    // Black image: every cell gets the largest dot (coverage 1). With no mark
    // band the image area touches the page edge, so polygon vertices (which sit
    // slightly outside the circle) must still land inside the page.
    let dir = tempfile::tempdir().unwrap();
    for (name, w, h) in [("tall.png", 200, 300), ("wide.png", 300, 200), ("square.png", 250, 250)] {
        let img = write_image(dir.path(), name, w, h, |_, _| 0);
        for angle in [0.0, 22.5, 45.0] {
            let mut req = request(img.clone(), dir.path(), "a5");
            req.angle_deg = Some(angle);
            let (c, _) = analyze(&req, &Limits::default()).unwrap();
            assert!(!c.output.registration_marks);
            let r = generate(&c, dir.path(), &Limits::default(), &GenerateOptions::default());
            assert!(r.is_ok(), "{name} at {angle}°: {:?}", r.err());
        }
    }
}
