//! Regression tests: `/Shading` resources and `sh` ops survive the round trip.
//! Before extract_shadings + the `sh` parser arm, gradients silently vanished
//! on every parse→save cycle (the resource was dropped and the op deleted by
//! the default `secure` save).

use printpdf::*;

fn gradient_doc() -> Vec<u8> {
    let shading_id = ShadingId("SH0".to_string());
    let mut doc = PdfDocument::new("shading roundtrip");
    doc.resources.shadings.map.insert(
        shading_id.clone(),
        Shading {
            geometry: ShadingGeometry::Axial {
                coords: [0.0, 0.0, 200.0, 0.0],
            },
            stops: vec![
                GradientStop { offset: 0.0, color: [1.0, 0.0, 0.0] },
                GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
            ],
            extend: (true, true),
        },
    );
    let page = PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![Op::PaintShading { id: shading_id }],
    );
    doc.pages.push(page);
    doc.save(&PdfSaveOptions::default(), &mut Vec::new())
}

#[test]
fn shading_and_sh_op_survive_roundtrip() {
    let bytes = gradient_doc();

    let mut warnings = Vec::new();
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut warnings)
        .expect("parse own gradient PDF");

    assert_eq!(
        parsed.resources.shadings.map.len(),
        1,
        "the /Shading resource must be parsed (warnings: {warnings:#?})"
    );
    let sh = parsed.resources.shadings.map.values().next().unwrap();
    assert!(matches!(sh.geometry, ShadingGeometry::Axial { .. }));
    assert_eq!(sh.extend, (true, true));
    assert!(sh.stops.len() >= 2, "stops: {:?}", sh.stops);
    assert_eq!(sh.stops.first().unwrap().color, [1.0, 0.0, 0.0]);
    assert_eq!(sh.stops.last().unwrap().color, [0.0, 0.0, 1.0]);

    let has_paint_op = parsed.pages[0]
        .ops
        .iter()
        .any(|op| matches!(op, Op::PaintShading { .. }));
    assert!(has_paint_op, "sh op must parse into Op::PaintShading");

    // Second round trip must not lose it either.
    let bytes2 = parsed.save(&PdfSaveOptions::default(), &mut warnings);
    let parsed2 = PdfDocument::parse(&bytes2, &PdfParseOptions::default(), &mut warnings)
        .expect("re-parse");
    assert_eq!(parsed2.resources.shadings.map.len(), 1);
    assert!(parsed2.pages[0]
        .ops
        .iter()
        .any(|op| matches!(op, Op::PaintShading { .. })));
}
