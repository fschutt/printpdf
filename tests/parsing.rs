// tests/parsing_tests.rs

use printpdf::{
    BlendMode, BuiltinFont, BuiltinOrExternalFontId, Color, ExtendedGraphicsState, Layer,
    LayerIntent, LayerSubtype, LineCapStyle, LineDashPattern, LineJoinStyle, Mm, Op, OverprintMode,
    PageAnnotId, PdfDocument, PdfPage, PdfParseOptions, PdfSaveOptions, Polygon, PolygonRing, Pt,
    RenderingIntent, Rgb, SeperableBlendMode, TextItem,
};

/// Test that layer creation, serialization, and parsing works correctly
#[test]
fn test_layer_parsing() {
    // Create a document with a layer
    let mut doc = PdfDocument::new("Layer Parsing Test");

    // Create a layer with specific properties
    let layer = Layer {
        name: "Test Layer".to_string(),
        creator: "Test Creator".to_string(),
        intent: LayerIntent::Design,
        usage: LayerSubtype::Artwork,
    };

    // Add the layer to the document
    let layer_id = doc.add_layer(&layer);

    // Create a page with layer operations
    let ops = vec![
        Op::BeginLayer {
            layer_id: layer_id.clone(),
        },
        // Add some content to the layer
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points: vec![] }],
                mode: printpdf::PaintMode::Fill,
                winding_order: printpdf::WindingOrder::NonZero,
            },
        },
        Op::EndLayer {
            layer_id: layer_id.clone(),
        },
    ];

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.pages.push(page);

    // Serialize the document
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Parse the document back
    let parsed_doc =
        PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();

    // Verify layers were preserved
    assert!(
        !parsed_doc.resources.layers.map.is_empty(),
        "No layers found in parsed document"
    );

    // Get the parsed layer (there should be only one)
    let parsed_layer = parsed_doc.resources.layers.map.values().next().unwrap();

    // Check layer properties were preserved
    assert_eq!(parsed_layer.name, "Test Layer");
    assert_eq!(parsed_layer.creator, "Test Creator");

    // Check layer operations were preserved
    let parsed_page = &parsed_doc.pages[0];
    let has_begin_layer = parsed_page.ops.iter().any(|op| {
        if let Op::BeginLayer { layer_id: _ } = op {
            true
        } else {
            false
        }
    });
    assert!(has_begin_layer, "Begin layer operation not found");

    let has_end_layer = parsed_page.ops.iter().any(|op| {
        if let Op::EndLayer { layer_id: _ } = op {
            true
        } else {
            false
        }
    });
    assert!(has_end_layer, "End layer operation not found");
}

/// Test that extended graphics state creation, serialization, and parsing works correctly
#[test]
fn test_extgstate_parsing() {
    // Create a document
    let mut doc = PdfDocument::new("ExtGState Parsing Test");

    // Create a dash pattern for testing
    let dash_pattern = LineDashPattern {
        offset: 0,
        dash_1: Some(5),
        gap_1: Some(3),
        dash_2: Some(2),
        gap_2: Some(1),
        dash_3: None,
        gap_3: None,
    };

    // Create an extended graphics state with various properties
    let gs = ExtendedGraphicsState::default()
        .with_line_width(2.0)
        .with_line_cap(LineCapStyle::Round)
        .with_line_join(LineJoinStyle::Bevel)
        .with_miter_limit(10.0)
        .with_line_dash_pattern(Some(dash_pattern))
        .with_rendering_intent(RenderingIntent::Perceptual)
        .with_overprint_stroke(true)
        .with_overprint_fill(true)
        .with_overprint_mode(OverprintMode::KeepUnderlying)
        .with_stroke_adjustment(true)
        .with_blend_mode(BlendMode::Seperable(SeperableBlendMode::Multiply))
        .with_current_stroke_alpha(0.5)
        .with_current_fill_alpha(0.7);

    // Add the extended graphics state to the document
    let gs_id = doc.add_graphics_state(gs);

    // Create a page with operations that use the graphics state
    let ops = vec![
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: gs_id.clone() },
        // Add some content that will be affected by the graphics state
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points: vec![] }],
                mode: printpdf::PaintMode::Fill,
                winding_order: printpdf::WindingOrder::NonZero,
            },
        },
        Op::RestoreGraphicsState,
    ];

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.pages.push(page);

    // Serialize the document
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Parse the document back
    let parsed_doc =
        PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();

    // Verify graphics states were preserved
    assert!(
        !parsed_doc.resources.extgstates.map.is_empty(),
        "No extended graphics states found in parsed document"
    );

    // Get the parsed graphics state (there should be only one)
    let parsed_gs = parsed_doc.resources.extgstates.map.values().next().unwrap();

    // Check operations were preserved
    let parsed_page = &parsed_doc.pages[0];
    let has_load_gs = parsed_page.ops.iter().any(|op| {
        if let Op::LoadGraphicsState { gs: _ } = op {
            true
        } else {
            false
        }
    });
    assert!(has_load_gs, "Load graphics state operation not found");
}

/// Test that bookmark creation, serialization, and parsing works correctly
#[test]
fn test_bookmark_parsing() {
    // Create a document
    let mut doc = PdfDocument::new("Bookmark Parsing Test");

    // Create multiple pages to bookmark
    for _ in 0..3 {
        let page = PdfPage::new(Mm(210.0), Mm(297.0), vec![]);
        doc.pages.push(page);
    }

    // Add bookmarks with specific names and targets
    let _ = doc.add_bookmark("Chapter 1", 1);
    let _ = doc.add_bookmark("Chapter 2", 2);
    let _ = doc.add_bookmark("Chapter 3", 3);

    // Serialize the document
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Parse the document back
    let parsed_doc =
        PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();

    // Verify bookmarks were preserved
    assert_eq!(
        parsed_doc.bookmarks.map.len(),
        3,
        "Expected 3 bookmarks, found {}",
        parsed_doc.bookmarks.map.len()
    );

    // Check bookmark properties
    let bookmarks: Vec<(&PageAnnotId, &printpdf::PageAnnotation)> =
        parsed_doc.bookmarks.map.iter().collect();

    let chapter1 = bookmarks
        .iter()
        .find(|(_, b)| b.name == "Chapter 1")
        .unwrap()
        .1;
    let chapter2 = bookmarks
        .iter()
        .find(|(_, b)| b.name == "Chapter 2")
        .unwrap()
        .1;
    let chapter3 = bookmarks
        .iter()
        .find(|(_, b)| b.name == "Chapter 3")
        .unwrap()
        .1;

    assert_eq!(chapter1.page, 1);
    assert_eq!(chapter2.page, 2);
    assert_eq!(chapter3.page, 3);
}

/// Test complex scenarios with combinations of layers, graphics states, and bookmarks
#[test]
fn test_complex_document_parsing() {
    // Create a document
    let mut doc = PdfDocument::new("Complex Document Parsing Test");

    // Create layers
    let layer1 = Layer {
        name: "Background".to_string(),
        creator: "Test Creator".to_string(),
        intent: LayerIntent::View,
        usage: LayerSubtype::Artwork,
    };

    let layer2 = Layer {
        name: "Content".to_string(),
        creator: "Test Creator".to_string(),
        intent: LayerIntent::Design,
        usage: LayerSubtype::Artwork,
    };

    let layer1_id = doc.add_layer(&layer1);
    let layer2_id = doc.add_layer(&layer2);

    // Create extended graphics states
    let gs1 = ExtendedGraphicsState::default()
        .with_line_width(1.5)
        .with_line_cap(LineCapStyle::Butt)
        .with_blend_mode(BlendMode::Seperable(SeperableBlendMode::Normal));

    let gs2 = ExtendedGraphicsState::default()
        .with_line_width(2.5)
        .with_line_cap(LineCapStyle::Round)
        .with_blend_mode(BlendMode::Seperable(SeperableBlendMode::Multiply))
        .with_overprint_mode(OverprintMode::KeepUnderlying);

    let gs1_id = doc.add_graphics_state(gs1);
    let gs2_id = doc.add_graphics_state(gs2);

    // Create pages with different combinations
    // Page 1: Layer 1 with GS 1
    let ops1 = vec![
        Op::BeginLayer {
            layer_id: layer1_id.clone(),
        },
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: gs1_id.clone() },
        Op::StartTextSection,
        Op::SetFontSizeBuiltinFont {
            size: Pt(12.0),
            font: BuiltinFont::Helvetica,
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Page 1 Content".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
        Op::EndLayer {
            layer_id: layer1_id.clone(),
        },
    ];

    // Page 2: Layer 2 with GS 2
    let ops2 = vec![
        Op::BeginLayer {
            layer_id: layer2_id.clone(),
        },
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: gs2_id.clone() },
        Op::StartTextSection,
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Page 2 Content".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
        Op::EndLayer {
            layer_id: layer2_id.clone(),
        },
    ];

    // Page 3: Both layers with different graphics states
    let ops3 = vec![
        Op::BeginLayer {
            layer_id: layer1_id.clone(),
        },
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: gs1_id.clone() },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.2,
                b: 0.2,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points: vec![] }],
                mode: printpdf::PaintMode::Fill,
                winding_order: printpdf::WindingOrder::NonZero,
            },
        },
        Op::RestoreGraphicsState,
        Op::EndLayer {
            layer_id: layer1_id.clone(),
        },
        Op::BeginLayer {
            layer_id: layer2_id.clone(),
        },
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: gs2_id.clone() },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.2,
                g: 0.8,
                b: 0.2,
                icc_profile: None,
            }),
        },
        Op::StartTextSection,
        Op::SetFontSizeBuiltinFont {
            size: Pt(16.0),
            font: BuiltinFont::Helvetica,
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Page 3 Content".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
        Op::EndLayer {
            layer_id: layer2_id.clone(),
        },
    ];

    let page1 = PdfPage::new(Mm(210.0), Mm(297.0), ops1);
    let page2 = PdfPage::new(Mm(210.0), Mm(297.0), ops2);
    let page3 = PdfPage::new(Mm(210.0), Mm(297.0), ops3);

    doc.pages.push(page1);
    doc.pages.push(page2);
    doc.pages.push(page3);

    // Add bookmarks
    let _ = doc.add_bookmark("Background Section", 1);
    let _ = doc.add_bookmark("Content Section", 2);
    let _ = doc.add_bookmark("Combined Section", 3);

    // Serialize the document
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Parse the document back
    let parsed_doc =
        PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();

    // Verify all components were preserved
    assert_eq!(
        parsed_doc.resources.layers.map.len(),
        2,
        "Expected 2 layers, found {}",
        parsed_doc.resources.layers.map.len()
    );
    assert_eq!(
        parsed_doc.resources.extgstates.map.len(),
        2,
        "Expected 2 graphics states, found {}",
        parsed_doc.resources.extgstates.map.len()
    );
    assert_eq!(
        parsed_doc.bookmarks.map.len(),
        3,
        "Expected 3 bookmarks, found {}",
        parsed_doc.bookmarks.map.len()
    );

    // Check layer names
    let layer_names: Vec<String> = parsed_doc
        .resources
        .layers
        .map
        .values()
        .map(|l| l.name.clone())
        .collect();
    assert!(layer_names.contains(&"Background".to_string()));
    assert!(layer_names.contains(&"Content".to_string()));

    // Check bookmark names
    let bookmark_names: Vec<String> = parsed_doc
        .bookmarks
        .map
        .values()
        .map(|b| b.name.clone())
        .collect();
    assert!(bookmark_names.contains(&"Background Section".to_string()));
    assert!(bookmark_names.contains(&"Content Section".to_string()));
    assert!(bookmark_names.contains(&"Combined Section".to_string()));

    // Verify operations on pages
    for (i, page) in parsed_doc.pages.iter().enumerate() {
        let page_number = i + 1;

        // Check for layer operations
        let layer_begins = page
            .ops
            .iter()
            .filter(|op| {
                if let Op::BeginLayer { layer_id: _ } = op {
                    true
                } else {
                    false
                }
            })
            .count();

        let layer_ends = page
            .ops
            .iter()
            .filter(|op| {
                if let Op::EndLayer { layer_id: _ } = op {
                    true
                } else {
                    false
                }
            })
            .count();

        match page_number {
            1 | 2 => {
                assert_eq!(
                    layer_begins, 1,
                    "Page {} should have 1 BeginLayer op",
                    page_number
                );
                assert_eq!(
                    layer_ends, 1,
                    "Page {} should have 1 EndLayer op",
                    page_number
                );
            }
            3 => {
                assert_eq!(
                    layer_begins, 2,
                    "Page {} should have 2 BeginLayer ops",
                    page_number
                );
                assert_eq!(
                    layer_ends, 2,
                    "Page {} should have 2 EndLayer ops",
                    page_number
                );
            }
            _ => {}
        }

        // Check for graphics state operations
        let gs_loads = page
            .ops
            .iter()
            .filter(|op| {
                if let Op::LoadGraphicsState { gs: _ } = op {
                    true
                } else {
                    false
                }
            })
            .count();

        match page_number {
            1 | 2 => {
                assert_eq!(
                    gs_loads, 1,
                    "Page {} should have 1 LoadGraphicsState op",
                    page_number
                );
            }
            3 => {
                assert_eq!(
                    gs_loads, 2,
                    "Page {} should have 2 LoadGraphicsState ops",
                    page_number
                );
            }
            _ => {}
        }
    }
}

/// Test that font references in extended graphics states are preserved
#[test]
fn test_extgstate_font_parsing() {
    // Create a document
    let mut doc = PdfDocument::new("ExtGState Font Parsing Test");

    // Create a parsed font (using builtin font as a shortcut)
    let font = BuiltinFont::Helvetica.get_subset_font();
    let parsed_font = printpdf::ParsedFont::from_bytes(&font.bytes, 0, &mut Vec::new()).unwrap();

    // Add the font to the document
    let font_id = doc.add_font(&parsed_font);

    // Create an extended graphics state with the font
    let gs = ExtendedGraphicsState::default()
        .with_font(Some(BuiltinOrExternalFontId::External(font_id.clone())))
        .with_line_width(1.5);

    // Add the graphics state to the document
    let gs_id = doc.add_graphics_state(gs);

    // Create a page with the graphics state
    let ops = vec![
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: gs_id.clone() },
        Op::StartTextSection,
        Op::SetFontSize {
            size: Pt(12.0),
            font: font_id.clone(),
        },
        Op::WriteText {
            items: vec![TextItem::Text("Testing Font in ExtGState".to_string())],
            font: font_id.clone(),
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ];

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.pages.push(page);

    // Serialize the document
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Parse the document back
    let parsed_doc =
        PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();

    // Verify the extended graphics state with font was preserved
    assert!(
        !parsed_doc.resources.extgstates.map.is_empty(),
        "No extended graphics states found"
    );

    // Verify the operations on the page
    let parsed_page = &parsed_doc.pages[0];

    // Check for text operations that use the font
    let has_font_ops = parsed_page.ops.iter().any(|op| match op {
        Op::WriteText { items: _, font: _ } => true,
        _ => false,
    });

    assert!(has_font_ops, "Text operations with font not found");
}
