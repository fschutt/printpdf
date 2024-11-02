# printpdf

[![CI](https://github.com/fschutt/printpdf/actions/workflows/ci.yaml/badge.svg)]
[![Dependencies](https://deps.rs/repo/github/fschutt/printpdf/status.svg)](https://deps.rs/repo/github/fschutt/printpdf)

`printpdf` is a Rust library for creating PDF documents.

[Crates.io](https://crates.io/crates/printpdf) | \
[Documentation](https://docs.rs/printpdf) | \
[Donate to this project](https://github.com/sponsors/fschutt)

## Features

Currently, printpdf can only write documents, not read them.

- Pages, Bookmarks, link annotations
- Layers (Illustrator like layers)
- Graphics (lines, shapes, bezier curves, SVG content)
- Images (uses the `image` crate)
- Fonts with Unicode support (uses `allsorts` for font shaping)
- Minifying file size (auto-subsetting fonts)
- HTML-based layout system using `azul-layout` (for easy generation of tables / page layout)
- Advanced graphics - overprint control, blending modes, etc.
- Advanced typography - character / word scaling and spacing, superscript, subscript, etc.
- Embedding SVGs (uses `svg2pdf` crate internally)

## Writing PDF

### Basic example

```rust
use printpdf::*;

let mut doc = PdfDocument::new("My first PDF");
let page1_contents = vec![Op::Marker { id: "debugging-marker".to_string() }];
let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
let pdf_bytes: Vec<u8> = doc.with_pages(page1).save_to_bytes();
```

### Graphics

```rust
use printpdf::*;

let mut doc = PdfDocument::new("My first PDF");

let line = Line {
    // Quadratic shape. The "false" determines if the next (following)
    // point is a bezier handle (for curves)
    // If you want holes, simply reorder the winding of the points to be
    // counterclockwise instead of clockwise.
    points: vec![
        (Point::new(Mm(100.0), Mm(100.0)), false),
        (Point::new(Mm(100.0), Mm(200.0)), false),
        (Point::new(Mm(300.0), Mm(200.0)), false),
        (Point::new(Mm(300.0), Mm(100.0)), false),
    ],
    is_closed: true,
};

// Triangle shape
let polygon = Polygon {
    rings: vec![vec![
        (Point::new(Mm(150.0), Mm(150.0)), false),
        (Point::new(Mm(150.0), Mm(250.0)), false),
        (Point::new(Mm(350.0), Mm(250.0)), false),
    ]],
    mode: PaintMode::FillStroke,
    winding_order: WindingOrder::NonZero,
};

// Graphics config
let fill_color = Color::Cmyk(Cmyk::new(0.0, 0.23, 0.0, 0.0, None));
let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));
let mut dash_pattern = LineDashPattern::default();
dash_pattern.dash_1 = Some(20);
let extgstate = doc.add_ExtendedGraphicsStateBuilder::new()
    .with_overprint_stroke(true)
    .with_blend_mode(BlendMode::multiply())
    .build();

let page1_contents = vec![
    // add line1 (square)
    Op::SetOutlineColor { col: Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None)) },
    Op::SetOutlineThickness { pt: Pt(10.0) },
    Op::DrawLine { line: line },

    // add line2 (triangle)
    Op::SaveGraphicsState,
    Op::LoadGraphicsState { gs: doc.add_graphics_state(extgstate) },
    Op::SetLineDashPattern { dash: dash_pattern },
    Op::SetLineJoinStyle { join: LineJoinStyle::Round },
    Op::SetLineCapStyle { cap: LineCapStyle::Round },
    Op::SetFillColor { col: fill_color_2 },
    Op::SetOutlineThickness { pt: Pt(15.0) },
    Op::SetOutlineColor { col: outline_color_2 },
    Op::DrawPolygon { polygon: polygon },
    Op::RestoreGraphicsState,
];

let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
let pdf_bytes: Vec<u8> = doc.with_pages(page1).save_to_bytes();
```

### Images

- Images only get compressed in release mode. You might get huge PDFs (6 or more MB) in debug mode.
-  To make this process faster, use `BufReader` instead of directly reading from the file.
- Scaling of images is implicitly done to fit one pixel = one dot at 300 dpi.

```rust
use printpdf::*;
use image::{Image, codecs::bmp::BmpDecoder};

fn main() {

    let mut doc = PdfDocument::new("My first PDF");
    let image_bytes = include_bytes!("assets/img/BMP_test.bmp");
    let image = Image::try_from(BmpDecoder::new(&mut image_file).unwrap()).unwrap();
    
    // In the PDF, an image is an `XObject`, in this case an `ImageXObject`.
    // returns a random 32-bit image ID
    let image_xobject_id = doc.add_image(image);

    let page1_contents = vec![
        Op::UseXObject { id: image_xobject_id.clone(), transform: XObjectTransform::default() }
    ];

    let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
    let pdf_bytes: Vec<u8> = doc.with_pages(page1).save_to_bytes();
}
```

### Fonts

```rust
use printpdf::*;

let mut doc = PdfDocument::new("My first PDF");

let font = ParsedFont::from_bytes(include_bytes!("assets/fonts/RobotoMedium.ttf")).unwrap();
let font_id = doc.add_font(font);
// let glyphs = font.shape(text);

let text_pos = Point { x: Mm(10.0).into(), y: Mm(100.0).into() }; // from bottom left
let page1_contents = vec![
    Op::SetFontSize { font: font_id.clone(), size: Pt(33.0) },
    Op::SetTextCursor { pos: text_pos }, 
    Op::SetLineHeight { lh: Pt(33.0) },
    Op::SetWordSpacing { percent: 3000.0 },
    Op::SetCharacterSpacing { multiplier: 10.0 },
    Op::WriteText { text: "Lorem ipsum".to_string(), font: font_id.clone() },
    Op::AddLineBreak,
    Op::WriteText { text: "dolor sit amet".to_string(), font: font_id.clone() },
    Op::AddLineBreak,
];

let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
let pdf_bytes: Vec<u8> = doc.with_pages(page1).save_to_bytes();
```

### Tables, HTML

For creating tables, etc. printpdf uses a basic layout system using the `azul-layout` crate.

```rust
// --features="html"

    // how to style footnote (empty = no footnotes)
    footnotes: "{ font-family:serif;font-size:14px; }",
    // how to style the header of a page (here: display the section-title attribute of a node + the page number)
    // by default `.pagenum` is set to display:none, i.e. don't display the page number
    header: "{ 
        min-height: 8mm; 
        font-family:sans-serif; 
        .section-title { font-weight: bold; } 
        .pagenum { display:block; position:absolute; top:5mm; left: 5mm; }
    }",
    footer: "", // no footer

let html = r#"
    <html>
        <head>

            <!-- optional: configure page header -->
            <header>

                <template>
                    <h4 class="section-header">Chapter {attr:chapter} * {attr:subsection}</h4>
                    <p class="pagenum">{builtin:pagenum}</p>
                </template>

                <style>
                    .section-header {
                        min-height: 8mm;
                        font-family: sans-serif;
                        color: #2e2e2e;
                        border-bottom: 1px solid black;
                        width: 100%;
                    }
                    .pagenum {
                        position: absolute;
                        top: 
                    }
                </style>
            </header>

            <!-- same for styling footers -->
            <footer>
                <template>
                    <hr/>
                </template>
            <footer/>
        </head>

        <!-- page content -->
        <body margins="10mm">
            <p style="color: red; font-family: sans-serif;" data-chapter="1" data-subsection="First subsection">Hello!</p>
            <div style="width:200px;height:200px;background:red;" data-chapter="1" data-subsection="Second subsection">
                <p>World!</p>
            </div>
        </body>

    </html>
"#;

let options = XmlRenderOptions {
    // named images to be used in the HTML, i.e. ["image1.png" => DecodedImage(image1_bytes)]
    images: BTreeMap::new(),
    // named fonts to be used in the HTML, i.e. ["Roboto" => DecodedImage(roboto_bytes)]
    fonts: BTreeMap::new(),
    // default page width, printpdf will auto-page-break
    page_width: Mm(210.0),
    // default page height
    page_height: Mm(297.0),
};

let mut doc = PdfDocument::new("My PDF");
let pages = crate::html::xml_to_pages(html, &options, &mut doc.resources).unwrap_or_defaul();
let pdf = doc.with_pages(pages).save_to_bytes();
```

## Goals and Roadmap

The goal of printpdf is to be a general-use PDF library, such as
libharu or similar. PDFs generated by printpdf should always adhere
to a PDF standard, except if you turn it off. Currently, only the
standard `PDF/X-3:2002` is covered (i.e. valid PDF according to Adobe
Acrobat). Over time, there will be more standards supported. Checking a
PDF for errors is currently only a stub.

The following features aren't implemented yet:

- Clipping
- Open Prepress Interface
- Halftoning images, Gradients, Patterns
- Forms, annotations
- Conformance / error checking for various PDF standards
- Embedded Javascript
- Reading PDF
- Completion of printpdf wiki

The printpdf wiki is live at: https://github.com/fschutt/printpdf/wiki

Here are some resources I found while working on this library:

- [`PDFXPlorer`, shows the DOM tree of a PDF, needs .NET 2.0](http://www.o2sol.com/pdfxplorer/download.htm)
- [Official PDF 1.7 reference](http://www.adobe.com/content/dam/Adobe/en/devnet/acrobat/pdfs/pdf_reference_1-7.pdf)
- [\[GERMAN\] How to embed unicode fonts in PDF](http://www.p2501.ch/pdf-howto/typographie/vollzugriff/direkt)
- [PDF X/1-a Validator](https://www.pdf-online.com/osa/validate.aspx)
- [PDF X/3 technical notes](http://www.pdfxreport.com/lib/exe/fetch.php?media=en:technote_pdfx_checks.pdf)

## Testing

Currently the testing is pretty much non-existent, because PDF is very hard to test.
This should change over time: Testing should be done in two stages. First, test
the individual PDF objects, if the conversion into a PDF object is done correctly.
The second stage is manual inspection of PDF objects via Adobe Preflight.

Put the tests of the first stage in /tests/mod.rs. The second stage tests are
better to be handled inside the plugins' mod.rs file. `printpdf` depends highly
on [lopdf](https://github.com/J-F-Liu/lopdf), so you can either construct your
test object against a real type or a debug string of your serialized type.
Either way is fine - you just have to check that the test object is conform to
what PDF expects.
