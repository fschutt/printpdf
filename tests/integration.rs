// tests/wasm_api_tests.rs

use std::collections::BTreeMap;

use printpdf::{
    wasm::structs::{
        document_to_bytes, resources_for_page, DocumentToBytesInput, ResourcesForPageInput,
    },
    BuiltinFont, Color, GeneratePdfOptions, Line, LinePoint, Mm, Op, PaintMode, PdfDocument,
    PdfPage, PdfParseOptions, PdfResources, PdfSaveOptions, PdfToSvgOptions, Point, Polygon,
    PolygonRing, Pt, Rgb, TextItem, WindingOrder, XObjectId, XObjectTransform,
};

#[test]
fn test_document_to_bytes() {
    // Create a simple document
    let mut doc = PdfDocument::new("test_document");
    let page = PdfPage::new(Mm(210.0), Mm(297.0), vec![]);
    doc.pages.push(page);

    // Prepare input for API
    let input = DocumentToBytesInput {
        doc,
        options: PdfSaveOptions::default(),
        return_byte_array: false,
    };

    let output = document_to_bytes(input).unwrap();

    assert!(!output.bytes.decode_bytes().unwrap_or_default().is_empty());
}

/* 
#[test]
fn test_page_to_svg() {
    // Create a document with various operations
    let mut doc = PdfDocument::new("page_to_svg");

    // Create a polygon
    let polygon = Polygon {
        rings: vec![PolygonRing {
            points: vec![
                LinePoint {
                    p: Point {
                        x: Pt(50.0),
                        y: Pt(50.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(150.0),
                        y: Pt(150.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(150.0),
                        y: Pt(50.0),
                    },
                    bezier: false,
                },
            ],
        }],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    };

    // Create a line
    let line = Line {
        points: vec![
            LinePoint {
                p: Point {
                    x: Pt(50.0),
                    y: Pt(200.0),
                },
                bezier: false,
            },
            LinePoint {
                p: Point {
                    x: Pt(150.0),
                    y: Pt(250.0),
                },
                bezier: false,
            },
        ],
        is_closed: false,
    };

    let ops = vec![
        Op::SaveGraphicsState,
        // Set fill color to red and draw polygon
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon { polygon },
        // Set outline color to blue and draw line
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                icc_profile: None,
            }),
        },
        Op::SetOutlineThickness { pt: Pt(2.0) },
        Op::DrawLine { line },
        // Add some text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point {
                x: Pt(50.0),
                y: Pt(300.0),
            },
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(24.0),
            font: BuiltinFont::Helvetica,
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Hello, World!".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ];

    // Create a page with various operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops.clone());

    doc.pages.push(page);

    // Serialize to bytes
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Deserialize back to document
    let deserialized_doc =
        PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();

    // Check that document has one page
    assert_eq!(deserialized_doc.pages.len(), 1);

    // Check that page has expected number of operations
    pretty_assertions::assert_eq!(deserialized_doc.pages[0].ops, ops);

    // Render page to SVG
    let svg = deserialized_doc.pages[0].to_svg(
        &PdfResources::default(),
        &PdfToSvgOptions::default(),
        &mut warnings,
    );

    // Check that SVG contains expected elements
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    // Should contain either a polygon or a path for the drawn polygon
    assert!(svg.contains("<polygon") || svg.contains("<path"));
    // Should contain text
    assert!(svg.contains("<text"));
    assert!(svg.contains("Hello, World!"));
}
*/

#[test]
fn test_resources_for_page() {
    // Create a page with an XObject reference
    let xobject_id = XObjectId::new();
    let page = PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![Op::UseXobject {
            id: xobject_id.clone(),
            transform: XObjectTransform::default(),
        }],
    );

    let input = ResourcesForPageInput { page };
    let data = resources_for_page(input).unwrap();

    // If successful, check that xobjects contains our ID
    assert!(data.xobjects.iter().any(|x| x.0 == xobject_id.0));
}

#[test]
fn test_html_to_document() {
    // This test might fail if HTML feature is not enabled

    // Simple HTML content
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Test Document</title>
    </head>
    <body>
        <h1>Hello, World!</h1>
        <p>This is a test document.</p>
    </body>
    </html>
    "#;

    let images = BTreeMap::default();
    let fonts = BTreeMap::default();
    let options = GeneratePdfOptions {
        page_height: Some(210.0),
        page_width: Some(297.0),
        font_embedding: Some(true),
        image_optimization: Some(printpdf::ImageOptimizationOptions::default()),
    };
    let mut warnings = Vec::new();
    let output = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
        .map_err(|e| {
            println!("HTML to PDF conversion failed or not supported: {e:?}");
            for w in warnings {
                println!("WARN: {:?}", w);
            }
        })
        .unwrap();

    let _ = std::fs::write(
        "./htmltest.pdf",
        output.save(&PdfSaveOptions::default(), &mut Vec::new()),
    );

    assert!(!output.pages.is_empty());
}
