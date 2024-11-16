# printpdf

[![CI](https://github.com/fschutt/printpdf/actions/workflows/ci.yaml/badge.svg)](https://github.com/fschutt/printpdf/actions/workflows/ci.yaml)
[![Dependencies](https://deps.rs/repo/github/fschutt/printpdf/status.svg)](https://deps.rs/repo/github/fschutt/printpdf)

`printpdf` is a Rust library for creating PDF documents.

[Website](https://fschutt.github.io/printpdf) | [Crates.io](https://crates.io/crates/printpdf) | [Documentation](https://docs.rs/printpdf) | [Donate](https://github.com/sponsors/fschutt)

> [!IMPORTANT]  
> HTML-to-PDF rendering is still experimental and WIP. 
> In doubt, position PDF elements manually instead.

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

See the WASM32 demo live at: https://fschutt.github.io/printpdf

## Writing PDF

### Basic example

```rust
use printpdf::*;

fn main() {
    let mut doc = PdfDocument::new("My first PDF");
    let page1_contents = vec![Op::Marker { id: "debugging-marker".to_string() }];
    let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
    let pdf_bytes: Vec<u8> = doc
        .with_pages(vec![page1])
        .save(&PdfSaveOptions::default());
}
```

### Graphics

```rust
use printpdf::*;

fn main() {
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

    let extgstate = ExtendedGraphicsStateBuilder::new()
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
        Op::SetFillColor { col: fill_color },
        Op::SetOutlineThickness { pt: Pt(15.0) },
        Op::SetOutlineColor { col: outline_color },
        Op::DrawPolygon { polygon: polygon },
        Op::RestoreGraphicsState,
    ];
    
    let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
    let pdf_bytes: Vec<u8> = doc
        .with_pages(vec![page1])
        .save(&PdfSaveOptions::default());
}
```

### Images

- Images only get compressed in release mode. You might get huge PDFs (6 or more MB) in debug mode.
-  To make this process faster, use `BufReader` instead of directly reading from the file.
- Scaling of images is implicitly done to fit one pixel = one dot at 300 dpi.

```rust
use printpdf::*;

fn main() {
    let mut doc = PdfDocument::new("My first PDF");
    let image_bytes = include_bytes!("assets/img/BMP_test.bmp");
    let image = RawImage::decode_from_bytes(image_bytes).unwrap(); // requires --feature bmp
    
    // In the PDF, an image is an `XObject`, identified by a unique `ImageId`
    let image_xobject_id = doc.add_image(image);

    let page1_contents = vec![
        Op::UseXObject { 
            id: image_xobject_id.clone(), 
            transform: XObjectTransform::default() 
        }
    ];

    let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
    let pdf_bytes: Vec<u8> = doc
        .with_pages(vec![page1])
        .save(&PdfSaveOptions::default());
}
```

### Fonts

```rust
use printpdf::*;

fn main() {

    let mut doc = PdfDocument::new("My first PDF");

    let roboto_bytes = include_bytes!("assets/fonts/RobotoMedium.ttf");
    let font = ParsedFont::from_bytes(roboto_bytes).unwrap();

    // If you need custom text shaping (uses the `allsorts` font shaper internally)
    // let glyphs = font.shape(text);

    // printpdf automatically keeps track of which fonts are used in the PDF
    let font_id = doc.add_font(font);

    let text_pos = Point { 
        x: Mm(10.0).into(), 
        y: Mm(100.0).into() 
    }; // from bottom left

    let page1_contents = vec![
        Op::SetLineHeight { lh: Pt(33.0) },
        Op::SetWordSpacing { percent: 3000.0 },
        Op::SetCharacterSpacing { multiplier: 10.0 },
        Op::SetTextCursor { pos: text_pos },

        // Op::WriteCodepoints { ... }
        // Op::WriteCodepointsWithKerning { ... }
        Op::WriteText { 
            text: "Lorem ipsum".to_string(), 
            font: font_id.clone(), 
            size: Pt(33.0) 
        },
        Op::AddLineBreak,
        Op::WriteText { 
            text: "dolor sit amet".to_string(), 
            font: font_id.clone(), 
            size: Pt(33.0) 
        },
        Op::AddLineBreak,
    ];

    let save_options = PdfSaveOptions {
        subset_fonts: true, // auto-subset fonts on save
        .. Default::default()
    };

    let page1 = PdfPage::new(Mm(10.0), Mm(250.0), page1_contents);
    let pdf_bytes: Vec<u8> = doc
        .with_pages(vec![page1])
        .save(&save_options);
}
```

### Tables, HTML

For creating tables, etc. printpdf uses a basic layout system, similar to wkhtmltopdf 
(although more limited in terms of features). It's good enough for basic page layouting, 
book rendering and reports / forms / etc. Includes automatic page-breaking.

Since printpdf supports WASM, there is an interactive demo at 
https://fschutt.github.io/printpdf - try playing with the XML.

See [SYNTAX.md](./SYNTAX.md) for the XML syntax description.

```rust
// needs --features="html"
use printpdf::*;

fn main() {

    // See https://fschutt.github.io/printpdf for an interactive WASM demo!

    let html = r#"
    <html>

        <!-- printpdf automatically breaks content into pages -->
        <body style="padding:10mm">
            <p style="color: red; font-family: sans-serif;" data-chapter="1" data-subsection="First subsection">Hello!</p>
            <div style="width:200px;height:200px;background:red;" data-chapter="1" data-subsection="Second subsection">
                <p>World!</p>
            </div>
        </body>

        <!-- configure header and footer for each page -->
        <head>
            <header>
                <h4 style="color: #2e2e2e;min-height: 8mm;">Chapter {attr:chapter} * {attr:subsection}</h4>
                <p style="position: absolute;top:5mm;left:5mm;">{builtin:pagenum}</p>
            </header>

            <footer>
                <hr/>
            <footer/>
        </head>
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

    let pdf_bytes = PdfDocument::new("My PDF")
        .with_html(html, &options).unwrap()
        .save(&PdfSaveOptions::default());
}
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
- [Official PDF 1.7 reference](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.7old.pdf)
- [\[GERMAN\] How to embed unicode fonts in PDF](http://www.p2501.ch/pdf-howto/typographie/vollzugriff/direkt)
- [PDF X/1-a Validator](https://www.pdf-online.com/osa/validate.aspx)
- [PDF X/3 technical notes](http://www.pdfxreport.com/lib/exe/fetch.php?media=en:technote_pdfx_checks.pdf)

## License / Support

Library is licensed MIT.

You can donate (one-time or recurrent) at https://github.com/sponsors/fschutt. Thanks!
