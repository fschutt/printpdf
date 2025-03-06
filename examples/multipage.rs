use printpdf::*;

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Multi-page Example");
    
    // Create a collection of pages
    let mut pages = Vec::new();
    
    // Create page 1: Title page
    let title_ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(105.0), Mm(160.0)), // Center of page
        },
        // Set text to be centered using text matrix
        Op::SetTextMatrix {
            matrix: TextMatrix::TranslateRotate(Pt(-150.0), Pt(0.0), 0.0),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(30.0), 
            font: BuiltinFont::TimesBold 
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.2,
                g: 0.3,
                b: 0.7,
                icc_profile: None,
            })
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("Multi-page PDF Example".to_string())], 
            font: BuiltinFont::TimesBold 
        },
        Op::AddLineBreak,
        Op::SetFontSizeBuiltinFont { 
            size: Pt(16.0), 
            font: BuiltinFont::TimesRoman 
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.3,
                g: 0.3,
                b: 0.3,
                icc_profile: None,
            })
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("Page 1: Cover Page".to_string())], 
            font: BuiltinFont::TimesRoman 
        },
        Op::EndTextSection,
    ];
    
    // Create page 2: Text content
    let page2_ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(24.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.5,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("Page 2: Content Page".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::AddLineBreak,
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("This is the second page of our multi-page PDF document.".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("It demonstrates how to create multiple pages with different content.".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
    ];
    
    // Create page 3: Shapes and graphics
    let page3_ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(24.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.7,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("Page 3: Graphics Page".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Draw a rectangle
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.3,
                b: 0.3,
                icc_profile: None,
            })
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(200.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(200.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(150.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(150.0) },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            }
        },
        
        // Draw a line
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.8,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(3.0) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(300.0), y: Pt(200.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(400.0), y: Pt(150.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
    ];
    
    // Add the pages to our collection
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), title_ops));
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), page2_ops));
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), page3_ops));
    
    // Add bookmarks for each page
    doc.add_bookmark("Cover Page", 1);
    doc.add_bookmark("Content Page", 2);
    doc.add_bookmark("Graphics Page", 3);
    
    // Save the PDF to a file
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    
    std::fs::write("./multipage_example.pdf", bytes).unwrap();
    println!("Created multipage_example.pdf");
}