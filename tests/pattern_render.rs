//! Renders pages with patterns with poppler's `pdftoppm` and checks the colors of single pixels:
//! a colored tiling pattern (with a matrix), uncolored tiling patterns, and a shading pattern as
//! a fill and as a stroke. The tests are skipped, with a message, when `pdftoppm` is not
//! installed.

use std::{path::PathBuf, process::Command};

use printpdf::*;

const SIZE: usize = 200;

fn fill(x: f32, y: f32, w: f32, h: f32) -> Op {
    Op::DrawRectangle {
        rectangle: Rect {
            mode: Some(PaintMode::Fill),
            ..Rect::from_xywh(Pt(x), Pt(y), Pt(w), Pt(h))
        },
    }
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::Rgb(Rgb::new(r, g, b, None))
}

/// A 200 x 200 pt page with the patterns of the tests.
///
/// - Top left: a colored pattern. A cell is 10 x 10 pt, red on the left half and blue on the
///   right half, and the matrix moves the cells 2 pt to the right.
/// - Top right: an uncolored pattern (the left half of each cell) in green.
/// - Middle left: the same uncolored pattern in magenta.
/// - Bottom right: a gradient from red to blue as a fill.
/// - Middle right: the same gradient as a stroke.
fn document() -> PdfDocument {
    let mut doc = PdfDocument::new("pattern render");

    let cell = Rect::from_wh(Pt(10.0), Pt(10.0));
    let colored = Pattern::colored_tiling(
        cell.clone(),
        10.0,
        10.0,
        vec![
            Op::SetFillColor { col: rgb(1.0, 0.0, 0.0) },
            fill(0.0, 0.0, 5.0, 10.0),
            Op::SetFillColor { col: rgb(0.0, 0.0, 1.0) },
            fill(5.0, 0.0, 5.0, 10.0),
        ],
    )
    .with_matrix([1.0, 0.0, 0.0, 1.0, 2.0, 0.0]);
    let uncolored =
        Pattern::uncolored_tiling(cell, 10.0, 10.0, vec![fill(0.0, 0.0, 5.0, 10.0)]);
    let gradient = Pattern::from_shading(Shading {
        geometry: ShadingGeometry::Axial { coords: [110.0, 0.0, 190.0, 0.0] },
        stops: vec![
            GradientStop { offset: 0.0, color: [1.0, 0.0, 0.0] },
            GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
        ],
        extend: (true, true),
    });
    let colored = doc.add_pattern(colored);
    let uncolored = doc.add_pattern(uncolored);
    let gradient = doc.add_pattern(gradient);

    let ops = vec![
        Op::SetFillPattern { id: colored, color: None },
        fill(10.0, 110.0, 80.0, 80.0),
        Op::SetFillPattern { id: uncolored.clone(), color: Some(rgb(0.0, 0.7, 0.0)) },
        fill(110.0, 110.0, 80.0, 80.0),
        Op::SetFillPattern { id: uncolored, color: Some(rgb(1.0, 0.0, 1.0)) },
        fill(10.0, 60.0, 80.0, 40.0),
        Op::SetFillPattern { id: gradient.clone(), color: None },
        fill(110.0, 10.0, 80.0, 40.0),
        Op::SetOutlinePattern { id: gradient, color: None },
        Op::SetOutlineThickness { pt: Pt(12.0) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point { x: Pt(110.0), y: Pt(80.0) }, bezier: false },
                    LinePoint { p: Point { x: Pt(190.0), y: Pt(80.0) }, bezier: false },
                ],
                is_closed: false,
            },
        },
    ];
    let mut page = PdfPage::new(Mm(10.0), Mm(10.0), ops);
    let area = Rect::from_wh(Pt(SIZE as f32), Pt(SIZE as f32));
    page.media_box = area.clone();
    page.trim_box = area.clone();
    page.crop_box = area;
    doc.pages.push(page);
    doc
}

struct Image {
    width: usize,
    pixels: Vec<u8>,
}

impl Image {
    /// The color at `(x, y)`, with `y` measured from the bottom of the page like in the PDF.
    fn at(&self, x: usize, y_from_bottom: usize) -> [u8; 3] {
        let row = SIZE - 1 - y_from_bottom;
        let start = (row * self.width + x) * 3;
        match self.pixels.get(start..start + 3) {
            Some([r, g, b]) => [*r, *g, *b],
            _ => panic!("pixel ({x}, {y_from_bottom}) is outside the image"),
        }
    }
}

/// Reads a binary PPM (`P6`).
fn read_ppm(bytes: &[u8]) -> Image {
    let mut pos = 0;
    let mut token = || {
        while bytes.get(pos).is_some_and(|b| b.is_ascii_whitespace()) {
            pos += 1;
        }
        let start = pos;
        while bytes.get(pos).is_some_and(|b| !b.is_ascii_whitespace()) {
            pos += 1;
        }
        String::from_utf8_lossy(&bytes[start..pos]).into_owned()
    };
    assert_eq!(token(), "P6");
    let width: usize = token().parse().expect("width");
    let height: usize = token().parse().expect("height");
    assert_eq!(token(), "255");
    assert_eq!((width, height), (SIZE, SIZE));
    // One whitespace byte follows the maximum value.
    Image { width, pixels: bytes[pos + 1..].to_vec() }
}

/// Renders the first page at 72 dpi, or `None` when `pdftoppm` is not installed.
fn render(pdf: &[u8], name: &str) -> Option<Image> {
    if Command::new("pdftoppm").arg("-v").output().is_err() {
        eprintln!("pdftoppm is not installed: {name} is skipped");
        return None;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let pdf_path = dir.join(format!("{name}.pdf"));
    let out_prefix = dir.join(name);
    std::fs::write(&pdf_path, pdf).expect("write pdf");
    let status = Command::new("pdftoppm")
        .args(["-r", "72", "-f", "1", "-l", "1", "-singlefile", "-aa", "no", "-aaVector", "no"])
        .arg(&pdf_path)
        .arg(&out_prefix)
        .status()
        .expect("run pdftoppm");
    assert!(status.success(), "pdftoppm failed");
    let ppm = std::fs::read(dir.join(format!("{name}.ppm"))).expect("read ppm");
    Some(read_ppm(&ppm))
}

fn is_near(pixel: [u8; 3], expected: [u8; 3], tolerance: u8) -> bool {
    pixel
        .iter()
        .zip(expected)
        .all(|(a, b)| a.abs_diff(b) <= tolerance)
}

#[track_caller]
fn assert_color(image: &Image, x: usize, y: usize, expected: [u8; 3], what: &str) {
    let pixel = image.at(x, y);
    assert!(
        is_near(pixel, expected, 40),
        "{what}: pixel ({x}, {y}) is {pixel:?}, expected about {expected:?}"
    );
}

const WHITE: [u8; 3] = [255, 255, 255];

fn check(image: &Image) {
    // The colored pattern. Cells start at x = 2 + 10 k: red for [2, 7), blue for [7, 12), and so
    // on, so the pixel at 21 is blue, where the same pattern without the matrix is red.
    assert_color(image, 14, 150, [255, 0, 0], "colored pattern, left half of a cell");
    assert_color(image, 19, 150, [0, 0, 255], "colored pattern, right half of a cell");
    assert_color(image, 21, 150, [0, 0, 255], "colored pattern, the matrix moves the cells");
    assert_color(image, 4, 150, WHITE, "outside the filled rectangle");

    // The uncolored pattern takes the color of the op that selects it, and the right half of
    // each cell is empty.
    assert_color(image, 112, 150, [0, 178, 0], "uncolored pattern, green");
    assert_color(image, 117, 150, WHITE, "uncolored pattern, gap");
    assert_color(image, 12, 80, [255, 0, 255], "uncolored pattern, magenta");
    assert_color(image, 17, 80, WHITE, "uncolored pattern, magenta gap");

    // The gradient as a fill: red at the left end, blue at the right end.
    let left = image.at(112, 30);
    let right = image.at(188, 30);
    assert!(left[0] > 200 && left[2] < 60, "gradient fill, left end: {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "gradient fill, right end: {right:?}");

    // The gradient as a stroke of 12 pt around y = 80, and nothing next to it.
    let left = image.at(113, 80);
    let right = image.at(187, 80);
    assert!(left[0] > 200 && left[2] < 60, "gradient stroke, left end: {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "gradient stroke, right end: {right:?}");
    assert_color(image, 150, 90, WHITE, "above the stroke");
}

#[test]
fn patterns_are_drawn_by_poppler() {
    let doc = document();
    let pdf = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    if let Some(image) = render(&pdf, "patterns_written") {
        check(&image);
    }
}

#[test]
fn patterns_are_drawn_after_a_round_trip() {
    let doc = document();
    let pdf = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    let mut warnings = Vec::new();
    let parsed =
        PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut warnings).expect("parse");
    let again = parsed.save(&PdfSaveOptions::default(), &mut Vec::new());
    if let Some(image) = render(&again, "patterns_round_trip") {
        check(&image);
    }
}

#[test]
fn the_svg_preview_draws_a_solid_color() {
    let doc = document();
    let mut warnings = Vec::new();
    let svg = doc
        .pages
        .first()
        .expect("a page")
        .to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut warnings);
    // The uncolored pattern is its color, and the gradient is its first color.
    assert!(svg.contains("rgb(255, 0, 255)"), "{svg}");
    assert!(svg.contains("rgb(255, 0, 0)"), "{svg}");
    // The colored pattern is the mean of red and blue.
    assert!(svg.contains("rgb(128, 0, 128)"), "{svg}");
    // One warning for each of the three patterns that are used.
    let pattern_warnings = warnings.iter().filter(|w| w.msg.contains("pattern")).count();
    assert_eq!(pattern_warnings, 3, "{warnings:?}");
}
