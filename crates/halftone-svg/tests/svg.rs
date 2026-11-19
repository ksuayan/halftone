use halftone_svg::{Document, Expect, preview_png, validate, write};
use halftone_types::geometry::{LineString, area_mm2};
use halftone_types::{MultiPolygon, Polygon, RegistrationTarget};

/// 10 mm squares (µm) with a 4 mm hole in the first.
fn artwork() -> MultiPolygon {
    let sq = |x: i32, y: i32, s: i32| LineString::from(vec![(x, y), (x + s, y), (x + s, y + s), (x, y + s), (x, y)]);
    MultiPolygon::new(vec![
        Polygon::new(sq(20_000, 20_000, 10_000), vec![sq(23_000, 23_000, 4_000)]),
        Polygon::new(sq(50_000, 20_000, 10_000), vec![]),
    ])
}

fn targets() -> Vec<RegistrationTarget> {
    vec![RegistrationTarget {
        cx_mm: 50.0,
        cy_mm: 6.0,
        radius_mm: 3.0,
        arm_mm: 5.0,
    }]
}

fn doc<'a>(art: &'a MultiPolygon, t: &'a [RegistrationTarget]) -> Document<'a> {
    Document {
        page_width_mm: 100.0,
        page_height_mm: 80.0,
        ink: "#000000",
        artwork: art,
        targets: t,
        mark_stroke_mm: 0.176,
    }
}

fn expect(marks: bool) -> Expect<'static> {
    Expect {
        page_width_mm: 100.0,
        page_height_mm: 80.0,
        ink: "#000000",
        registration_marks: marks,
    }
}

#[test]
fn writes_valid_minimal_svg() {
    let (art, t) = (artwork(), targets());
    let svg = write(&doc(&art, &t));
    assert!(svg.contains("width=\"100mm\" height=\"80mm\" viewBox=\"0 0 100000 80000\""));
    let report = validate(&svg, &expect(true)).unwrap();
    assert_eq!(report.subpaths, 3);
    // Without marks the extra path must be absent, and expectations are enforced.
    let svg_plain = write(&doc(&art, &[]));
    assert!(validate(&svg_plain, &expect(false)).is_ok());
    assert!(validate(&svg_plain, &expect(true)).is_err());
}

#[test]
fn validator_rejects_bad_documents() {
    let (art, t) = (artwork(), targets());
    let good = write(&doc(&art, &t));
    let cases = [
        good.replace("width=\"100mm\"", "width=\"100px\""),
        good.replace("fill=\"#000000\" fill-rule", "fill=\"#ff0000\" fill-rule"),
        good.replace("</svg>", "<image href=\"x.png\"/></svg>"),
        good.replace("M20000 20000", "M200000 20000"),
        good.replace("<svg", "<svg><g>"),
    ];
    for (k, bad) in cases.iter().enumerate() {
        assert!(validate(bad, &expect(true)).is_err(), "case {k} should fail");
    }
}

#[test]
fn renderer_agrees_with_geometry_area() {
    let art = artwork();
    let svg = write(&doc(&art, &[]));
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    let px_per_mm = 20.0;
    let s = (px_per_mm * 25.4 / 96.0) as f32;
    let mut pm = resvg::tiny_skia::Pixmap::new(2000, 1600).unwrap();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pm.as_mut());
    let ink: f64 = pm.pixels().iter().map(|p| p.alpha() as f64 / 255.0).sum::<f64>() / (px_per_mm * px_per_mm);
    assert!(
        (ink - area_mm2(&art)).abs() < 0.05,
        "rendered {ink} mm² vs {} mm²",
        area_mm2(&art)
    );
}

#[test]
fn preview_is_png_on_white() {
    let (art, t) = (artwork(), targets());
    let png = preview_png(&doc(&art, &t), 100.0).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let pm = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
    assert_eq!((pm.width(), pm.height()), (394, 315));
    let corner = pm.pixel(1, pm.height() - 2).unwrap();
    assert_eq!((corner.red(), corner.alpha()), (255, 255));
}
