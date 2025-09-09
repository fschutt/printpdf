//! Simple XHTML rendering test.
//!
//! Should render as (ASCII ART):
//!
//! ```
//! Test passes if the filled blue square touches the upper-left corner of the black box.
//!
//! +-----------------------+
//! |BBBBBBBB               |
//! |BBBBBBBB               |
//! |BBBBBBBB               |
//! |BBBBBBBB               |
//! |                       |
//! |                       |
//! |                       |
//! +-----------------------+
//! ```
//!
//! ### Explanation for an LLM
//!  
//! This ASCII art represents an image that contains two main elements: a line of text and a simple
//! diagram.  
//! 1. **Text:** The line at the top reads, "Test passes the a filled blue square touches the
//!    upper-left corner of the black box." This text describes a condition for a test to pass.
//!    (Note: There appears to be a typo, "the a" likely should be "if a" //! or "that a").
//!  
//! 2. **Diagram:**
//! * **The Black Box:** This is represented by the large, hollow rectangle drawn with `+`, `-`, and
//!   `|` characters. It serves as a container or a frame of reference.
//! * **The Filled Blue Square:** This is represented by the block of `B` characters (for "Blue") in
//!   the top-left corner of the black box. This square is solid or "filled."
//!  
//! 3. **Relationship:** The key aspect of the diagram is the positioning of the blue square. It is
//!    placed exactly in the upper-left corner of the black box, with its top and left edges aligned
//!    perfectly with the box's borders.
//!  
//! **Conclusion:** The image visually demonstrates a successful test case. The diagram perfectly
//! matches the condition described in the text, confirming that the "Test passes" because "a filled
//! blue square touches the upper-left corner of the black box."

use std::collections::BTreeMap;

use printpdf::*;

const XHTML: &str = r#"
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
    <head>
        <title>CSS Test: Absolutely positioned, non-replaced elements, static position of fixed element</title>
        <link rel="author" title="Microsoft" href="http://www.microsoft.com/" />
        <link rel="help" href="http://www.w3.org/TR/CSS21/visudet.html#abs-non-replaced-height" />
        <meta name="flags" content="" />
        <meta name="assert" content="The calculation of static position is based on initial containing block when there is a fixed positioned element." />
        <style type="text/css">* { margin: 0; padding: 0 }
            html, body, p
            {
                margin: 0;
                padding: 0;
            }
            #div1
            {
                border: solid black;
                height: 2in;
                position: absolute;
                top: 1in;
                width: 3in;
            }
            div div
            {
                background: blue;
                height: 1in;
                position: fixed;
                width: 1in;
            }
        </style>
    </head>
    <body>
        <p>Test passes if the filled blue square touches the upper-left corner of the black box.</p>
        <div id="div1">
            <div></div>
        </div>
    </body>
</html>
"#;

// Unused, but rough layout of the rendered PDF
const _TARGET_PDF: &str = r#"
%PDF-1.7
%âãÏÓ

1 0 obj
<<
  /Type /Catalog
  /Pages 2 0 R
>>
endobj

2 0 obj
<<
  /Type /Pages
  /Kids [3 0 R]
  /Count 1
>>
endobj

3 0 obj
<<
  /Type /Page
  /Parent 2 0 R
  /MediaBox [0 0 612 792] % US Letter: 8.5x11 inches
  /Contents 4 0 R
  /Resources <<
    /Font <<
      /F1 <<
        /Type /Font
        /Subtype /Type1
        /BaseFont /TimesRoman
      >>
    >>
  >>
>>
endobj

4 0 obj
<< /Length 303 >>
stream
% --- Page Content Stream Begins ---

% 1. Draw the text
BT
  /F1 12 Tf
  72 750 Td
  (Test passes if the filled blue square touches the upper-left corner of the black box.) Tj
ET

% 2. Draw the black box (#div1)
% CSS: position:absolute; top:1in; width:3in; height:2in; border:solid black;
0 G                       % Set stroke color to black (grayscale)
1 w                       % Set line width
72 576 216 144 re         % Define rect: x=72, y=(792-72-144), w=216, h=144
S                         % Stroke path

% 3. Draw the blue square (inner div)
% CSS: position:fixed; width:1in; height:1in; background:blue;
% Position is determined by static flow inside #div1
0 0 1 rg                  % Set fill color to blue
72 648 72 72 re           % Define rect: x=72, y=(792-72-72), w=72, h=72
f                         % Fill path

% --- Page Content Stream Ends ---
endstream
endobj

xref
0 5
0000000000 65535 f 
0000000015 00000 n 
0000000060 00000 n 
0000000111 00000 n 
0000000288 00000 n 

trailer
<<
  /Size 5
  /Root 1 0 R
>>
startxref
644
%%EOF
"#;

fn main() {
    // Convert the HTML to PDF pages
    let mut warnings = Vec::new();
    let rendered = PdfDocument::from_html(
        &XHTML,
        &BTreeMap::new(), // fonts - should use builtin TimesRoman font
        &BTreeMap::new(), // images - no images used
        &GeneratePdfOptions::default(),
        &mut warnings,
    );

    if let Ok(r) = rendered.as_ref() {
        let bytes = r.save(&PdfSaveOptions::default(), &mut Vec::new());
        let s = String::from_utf8_lossy(&bytes);
        println!("--- rendered PDF: ---");
        println!("{s}");
        println!("---");
    }

    println!("warnings: {warnings:#?}");

    let rendered = rendered.unwrap();

    // --- Verification Steps ---

    // 1. Ensure exactly one page was created
    assert_eq!(
        rendered.pages.len(),
        1,
        "Expected one page to be rendered, but found {}",
        rendered.pages.len()
    );
    let page = &rendered.pages[0];

    // 2. Verify the text content
    let text_content = page.extract_text(&rendered.resources).join(" ");
    let expected_text =
        "Test passes if the filled blue square touches the upper-left corner of the black box.";
    assert!(
        text_content.contains(expected_text),
        "PDF content did not contain the expected text.\nExpected to find: '{}'\nFound: '{}'",
        expected_text,
        text_content
    );

    // 3. Find all rectangular shapes and their associated fill/stroke colors
    let mut last_fill_color: Option<Color> = None;
    let mut last_stroke_color: Option<Color> = None;
    let mut found_rects = Vec::new();

    for op in &page.ops {
        match op {
            Op::SetFillColor { col } => last_fill_color = Some(col.clone()),
            Op::SetOutlineColor { col } => last_stroke_color = Some(col.clone()),
            Op::DrawPolygon { polygon } => {
                // We are only interested in shapes that are perfect rectangles
                for r in polygon.rings.iter() {
                    found_rects.push((
                        r.bbox(),
                        last_fill_color.clone(),
                        last_stroke_color.clone(),
                    ));
                }
            }
            _ => {}
        }
    }

    // 4. Identify the specific blue square and black box from the shapes we found
    const TOLERANCE: f32 = 2.0; // Use a small tolerance for float comparisons

    let blue_square: Option<Rect> = found_rects.iter().find_map(|(rect, fill, _stroke)| {
        if let Some(Color::Rgb(rgb)) = fill {
            // Check for blue color and 1x1 inch size (72x72 pt)
            if rgb.r < 0.1
                && rgb.g < 0.1
                && rgb.b > 0.9
                && (rect.width.0 - 72.0).abs() < TOLERANCE
                && (rect.height.0 - 72.0).abs() < TOLERANCE
            {
                return Some(*rect);
            }
        }
        None
    });

    let black_box: Option<Rect> = found_rects.iter().find_map(|(rect, _fill, stroke)| {
        if let Some(color) = stroke {
            let is_black = match color {
                Color::Rgb(rgb) => rgb.r < 0.1 && rgb.g < 0.1 && rgb.b < 0.1,
                Color::Greyscale(g) => g.percent < 0.1,
                _ => false,
            };
            // Check for black color and 3x2 inch size (216x144 pt)
            if is_black
                && (rect.width.0 - 216.0).abs() < TOLERANCE
                && (rect.height.0 - 144.0).abs() < TOLERANCE
            {
                return Some(*rect);
            }
        }
        None
    });

    // 5. Assert that both shapes were successfully identified
    let blue_square = blue_square.expect("Did not find a 1x1 inch filled blue square in the PDF.");
    let black_box = black_box.expect("Did not find a 3x2 inch stroked black box in the PDF.");

    // 6. Final assertion: Verify that the blue square is in the top-left corner of the black box
    // The bounding box gives lower-left (ll) and upper-right (ur) corners.
    // top = ll.y + height
    // left = ll.x
    let blue_top = blue_square.lower_left().y.0 + blue_square.height.0;
    let blue_left = blue_square.lower_left().x.0;

    let black_top = black_box.lower_left().y.0 + black_box.height.0;
    let black_left = black_box.lower_left().x.0;

    assert!(
        (blue_left - black_left).abs() < TOLERANCE,
        "Alignment failed: Blue square is not left-aligned with black box. Blue left: {}, Black \
         left: {}",
        blue_left,
        black_left
    );
    assert!(
        (blue_top - black_top).abs() < TOLERANCE,
        "Alignment failed: Blue square is not top-aligned with black box. Blue top: {}, Black \
         top: {}",
        blue_top,
        black_top
    );

    println!("Success: PDF content verification passed!");
    println!("- Found expected text.");
    println!(
        "- Found 1x1 inch blue square at ({:.2}, {:.2})",
        blue_square.lower_left().x.0,
        blue_square.lower_left().y.0
    );
    println!(
        "- Found 3x2 inch black box at ({:.2}, {:.2})",
        black_box.lower_left().x.0,
        black_box.lower_left().y.0
    );
    println!("- Verified top-left corner alignment.");
}
