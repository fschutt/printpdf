//! Patterns as fill and stroke colors.
//!
//! A pattern is a color that varies over the page. This example writes `patterns.pdf` with
//!
//! - a colored tiling pattern (a repeated drawing that sets its own colors),
//! - an uncolored tiling pattern in two colors (the drawing is a shape, the color comes from the
//!   op that selects the pattern),
//! - a gradient used as a fill and as a stroke (a shading pattern),
//!
//! then reads the file again and prints the patterns it finds.
//!
//! ```text
//! cargo run --example patterns
//! ```

use printpdf::{
    Color, GradientStop, Line, LinePoint, Mm, Op, PaintMode, Pattern, PdfDocument, PdfPage,
    PdfParseOptions, PdfSaveOptions, Point, Pt, Rect, Rgb, Shading, ShadingGeometry,
};
use std::{error::Error, fs};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Op {
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

fn main() -> Result<(), Box<dyn Error>> {
    let mut doc = PdfDocument::new("Patterns");

    // A 12 x 12 pt cell with an orange square and a blue one. The cell is repeated every 12 pt.
    let checks = doc.add_pattern(Pattern::colored_tiling(
        Rect::from_wh(Pt(12.0), Pt(12.0)),
        12.0,
        12.0,
        vec![
            Op::SetFillColor { col: rgb(0.95, 0.5, 0.1) },
            rect(0.0, 0.0, 6.0, 6.0),
            Op::SetFillColor { col: rgb(0.1, 0.3, 0.8) },
            rect(6.0, 6.0, 6.0, 6.0),
        ],
    ));

    // A 8 x 8 pt cell with a square in the middle. The cell sets no color: the ops that select
    // the pattern give it one.
    let dots = doc.add_pattern(
        Pattern::uncolored_tiling(
            Rect::from_wh(Pt(8.0), Pt(8.0)),
            8.0,
            8.0,
            vec![rect(2.0, 2.0, 4.0, 4.0)],
        )
        // Turned by 30 degrees.
        .with_matrix([0.866, 0.5, -0.5, 0.866, 0.0, 0.0]),
    );

    // A gradient from red to blue between two points, in the coordinates of the page.
    let gradient = doc.add_pattern(Pattern::from_shading(Shading {
        geometry: ShadingGeometry::Axial { coords: [60.0, 0.0, 400.0, 0.0] },
        stops: vec![
            GradientStop { offset: 0.0, color: [0.9, 0.1, 0.1] },
            GradientStop { offset: 1.0, color: [0.1, 0.1, 0.9] },
        ],
        extend: (true, true),
    }));

    let ops = vec![
        Op::SetFillPattern { id: checks, color: None },
        rect(60.0, 600.0, 200.0, 120.0),
        Op::SetFillPattern { id: dots.clone(), color: Some(rgb(0.0, 0.6, 0.2)) },
        rect(300.0, 600.0, 200.0, 120.0),
        Op::SetFillPattern { id: dots, color: Some(rgb(0.8, 0.0, 0.5)) },
        rect(300.0, 440.0, 200.0, 120.0),
        Op::SetFillPattern { id: gradient.clone(), color: None },
        rect(60.0, 440.0, 200.0, 120.0),
        // The same gradient as the color of a thick line.
        Op::SetOutlinePattern { id: gradient, color: None },
        Op::SetOutlineThickness { pt: Pt(24.0) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point { x: Pt(60.0), y: Pt(380.0) }, bezier: false },
                    LinePoint { p: Point { x: Pt(500.0), y: Pt(380.0) }, bezier: false },
                ],
                is_closed: false,
            },
        },
    ];

    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(Mm(210.0), Mm(297.0), ops)])
        .save(&PdfSaveOptions::default(), &mut warnings);
    fs::write("patterns.pdf", &pdf)?;

    // Reading the file gives the patterns back.
    let parsed = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut warnings)?;
    for (id, pattern) in &parsed.resources.patterns.map {
        let kind = match pattern {
            Pattern::Tiling { .. } => "tiling pattern",
            Pattern::Shading { .. } => "shading pattern",
            Pattern::External(_) => "pattern kept as it is",
        };
        println!("{}: {kind}", id.0);
    }

    for warning in warnings {
        eprintln!("{}", warning.msg);
    }
    Ok(())
}
