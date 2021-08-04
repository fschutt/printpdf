# printpdf

[![Travis CI](https://travis-ci.org/fschutt/printpdf.svg?branch=master)](https://travis-ci.org/fschutt/printpdf) [![Appveyor](https://ci.appveyor.com/api/projects/status/2ioc0wopm5a8ixgm?svg=true)](https://ci.appveyor.com/project/fschutt/printpdf)
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Ffschutt%2Fprintpdf.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2Ffschutt%2Fprintpdf?ref=badge_shield)

`printpdf` is a library designed for creating printable PDF documents. 

[Crates.io](https://crates.io/crates/printpdf) | [Documentation](https://docs.rs/printpdf)

```toml,ignore
[dependencies]
printpdf = "0.3.2"
```

## Features

Currently, printpdf can only create new documents and write them, it cannot load existing documents yet.

- Page generation
- Layers (Illustrator like layers)
- Graphics (lines, shapes, bezier curves)
- Images (currently BMP/JPG/PNG only or generate your own images)
- Embedded fonts (TTF and OTF) with Unicode support
- Advanced graphics - overprint control, blending modes, etc.
- Advanced typography - character scaling, character spacing, superscript, subscript, outlining, etc.
- PDF layers (you should be able to open the PDF in Illustrator and have the layers appear)

Note: `printpdf` only implements the PDF spec, nothing more. If you more high-level PDF generation, 
take a look at [`genpdf`](https://crates.io/crates/genpdf), which is built on top of `printpdf`

## Getting started

### Writing PDF

#### Simple page

```rust
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
let (page2, layer1) = doc.add_page(Mm(10.0), Mm(250.0),"Page 2, Layer 1");

doc.save(&mut BufWriter::new(File::create("test_working.pdf").unwrap())).unwrap();
```

#### Adding graphical shapes

```rust
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::iter::FromIterator;

let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(297.0), Mm(210.0), "Layer 1");
let current_layer = doc.get_page(page1).get_layer(layer1);

// Quadratic shape. The "false" determines if the next (following)
// point is a bezier handle (for curves)
// If you want holes, simply reorder the winding of the points to be
// counterclockwise instead of clockwise.
let points1 = vec![(Point::new(Mm(100.0), Mm(100.0)), false),
                   (Point::new(Mm(100.0), Mm(200.0)), false),
                   (Point::new(Mm(300.0), Mm(200.0)), false),
                   (Point::new(Mm(300.0), Mm(100.0)), false)];

// Is the shape stroked? Is the shape closed? Is the shape filled?
let line1 = Line {
    points: points1,
    is_closed: true,
    has_fill: true,
    has_stroke: true,
    is_clipping_path: false,
};

// Triangle shape
// Note: Line is invisible by default, the previous method of
// constructing a line is recommended!
let mut line2 = Line::from_iter(vec![
    (Point::new(Mm(150.0), Mm(150.0)), false),
    (Point::new(Mm(150.0), Mm(250.0)), false),
    (Point::new(Mm(350.0), Mm(250.0)), false)]);

line2.set_stroke(true);
line2.set_closed(false);
line2.set_fill(false);
line2.set_as_clipping_path(false);

let fill_color = Color::Cmyk(Cmyk::new(0.0, 0.23, 0.0, 0.0, None));
let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));
let mut dash_pattern = LineDashPattern::default();
dash_pattern.dash_1 = Some(20);

current_layer.set_fill_color(fill_color);
current_layer.set_outline_color(outline_color);
current_layer.set_outline_thickness(10.0);

// Draw first line
current_layer.add_shape(line1);

let fill_color_2 = Color::Cmyk(Cmyk::new(0.0, 0.0, 0.0, 0.0, None));
let outline_color_2 = Color::Greyscale(Greyscale::new(0.45, None));

// More advanced graphical options
current_layer.set_overprint_stroke(true);
current_layer.set_blend_mode(BlendMode::Seperable(SeperableBlendMode::Multiply));
current_layer.set_line_dash_pattern(dash_pattern);
current_layer.set_line_cap_style(LineCapStyle::Round);

current_layer.set_fill_color(fill_color_2);
current_layer.set_outline_color(outline_color_2);
current_layer.set_outline_thickness(15.0);

// draw second line
current_layer.add_shape(line2);
```

#### Adding images

Note: Images only get compressed in release mode. You might get huge PDFs (6 or more MB) in
debug mode. In release mode, the compression makes these files much smaller (~ 100 - 200 KB).

To make this process faster, use `BufReader` instead of directly reading from the file.
Images are currently not a top priority.

Scaling of images is implicitly done to fit one pixel = one dot at 300 dpi.

```rust
extern crate printpdf;

// imports the `image` library with the exact version that we are using
use printpdf::*;

use std::convert::From;
use std::fs::File;

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // currently, the only reliable file formats are bmp/jpeg/png
    // this is an issue of the image library, not a fault of printpdf
    let mut image_file = File::open("assets/img/BMP_test.bmp").unwrap();
    let image = Image::try_from(image::bmp::BmpDecoder::new(&mut image_file).unwrap()).unwrap();

    // translate x, translate y, rotate, scale x, scale y
    // by default, an image is optimized to 300 DPI (if scale is None)
    // rotations and translations are always in relation to the lower left corner
    image.add_to_layer(current_layer.clone(), None, None, None, None, None, None);

    // you can also construct images manually from your data:
    let mut image_file_2 = ImageXObject {
        width: Px(200),
        height: Px(200),
        color_space: ColorSpace::Greyscale,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        /* put your bytes here. Make sure the total number of bytes =
           width * height * (bytes per component * number of components)
           (e.g. 2 (bytes) x 3 (colors) for RGB 16bit) */
        image_data: Vec::new(),
        image_filter: None, /* does not work yet */
        clipping_bbox: None, /* doesn't work either, untested */
    };

    let image2 = Image::from(image_file_2);
}
```

#### Adding fonts

Note: Fonts are shared between pages. This means that they are added to the document first
and then a reference to this one object can be passed to multiple pages. This is different to
images, for example, which can only be used once on the page they are created on (since that's
the most common use-case).

```rust
use printpdf::*;
use std::fs::File;

let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
let current_layer = doc.get_page(page1).get_layer(layer1);

let text = "Lorem ipsum";
let text2 = "unicode: стуфхfцчшщъыьэюя";

let font = doc.add_external_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
let font2 = doc.add_external_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();

// text, font size, x from left edge, y from bottom edge, font
current_layer.use_text(text, 48, Mm(200.0), Mm(200.0), &font);

// For more complex layout of text, you can use functions
// defined on the PdfLayerReference
// Make sure to wrap your commands
// in a `begin_text_section()` and `end_text_section()` wrapper
current_layer.begin_text_section();

    // setup the general fonts.
    // see the docs for these functions for details
    current_layer.set_font(&font2, 33);
    current_layer.set_text_cursor(Mm(10.0), Mm(10.0));
    current_layer.set_line_height(33);
    current_layer.set_word_spacing(3000);
    current_layer.set_character_spacing(10);
    current_layer.set_text_rendering_mode(TextRenderingMode::Stroke);

    // write two lines (one line break)
    current_layer.write_text(text.clone(), &font2);
    current_layer.add_line_break();
    current_layer.write_text(text2.clone(), &font2);
    current_layer.add_line_break();

    // write one line, but write text2 in superscript
    current_layer.write_text(text.clone(), &font2);
    current_layer.set_line_offset(10);
    current_layer.write_text(text2.clone(), &font2);

current_layer.end_text_section();
```

## Optimiziation

### Minimizing the size of generated PDFs

- By default, the PDF adherese to a "PDF conformance level", usually the PDF-X 1.4 Standard. 
  This means that the PDF includes a full ICC profile file (which is around 500KB large). To turn it off,
  see the [no_icc example](https://github.com/fschutt/printpdf/blob/a6fa46dad0f273cfd181eeebec7dc61e02559b4f/examples/no_icc.rs):

```rust
let (mut doc, _page1, _layer1) = PdfDocument::new("printpdf no_icc test", Mm(297.0), Mm(210.0), "Layer 1");
doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
  requires_icc_profile: false,
  requires_xmp_metadata: false,
    .. Default::default()
}));
```

- In debug mode, the images, streams and fonts are not compressed for easier debugging. Try building
  in release mode to optimize the size further.

## Changelog

See the CHANGELOG.md file.

## Further reading

The `PdfDocument` is hidden behind a `PdfDocumentReference`, which locks
the things you can do behind a facade. Pretty much all functions operate
on a `PdfLayerReference`, so that would be where to look for existing
functions or where to implement new functions. The `PdfDocumentReference`
is a reference-counted document. It uses the pages and layers for inner
mutablility, because
I ran into borrowing issues with the document. __IMPORTANT:__ All functions
that mutate the state of the document, "borrow" the document mutably for
the duration of the function. It is important that you don't borrow the
document twice (your program will crash if you do so). I have prevented
this wherever possible, by making the document only public to the crate
so you cannot lock it from outside of this library.

Images have to be added to the pages resources before using them. Meaning,
you can only use an image on the page that you added it to. Otherwise,
you may end up with a corrupt PDF.

Fonts are embedded using `freetype`. There is a `rusttype` branch in this
repository, but `rusttype` does fails to get the height of an unscaled
font correctly, so that's why you currently have to use `freetype`

Please report issues if you have any, especially if you see `BorrowMut`
errors (they should not happen). Kerning is currently not done, because
neither `freetype` nor `rusttype` can reliably read kerning data.
However, "correct" kerning / placement requires a full font shaping
engine, etc. This would be a completely different project.

For learning how a PDF is actually made, please read the
[wiki](https://github.com/fschutt/printpdf/wiki) (currently not
completely finished). When I began making this library, these resources
were not available anywhere, so I hope to help other people
with these topics. Reading the wiki is essential if you want to
contribute to this library.

## Goals and Roadmap

The goal of printpdf is to be a general-use PDF library, such as
libharu or similar. PDFs generated by printpdf should always adhere
to a PDF standard, except if you turn it off. Currently, only the
standard `PDF/X-3:2002` is covered (i.e. valid PDF according to Adobe
Acrobat). Over time, there will be more standards supported. Checking a
PDF for errors is currently only a stub.

### Planned features / Not done yet

The following features aren't implemented yet, most
- Clipping
- Aligning / layouting text
- Open Prepress Interface
- Halftoning images, Gradients, Patterns
- SVG / instantiated content
- Forms, annotations
- Bookmarks / Table of contents
- Conformance / error checking for various PDF standards
- Embedded Javascript
- Reading PDF
- Completion of printpdf wiki

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

## Useful links

Here are some resources I found while working on this library:

[`PDFXPlorer`, shows the DOM tree of a PDF, needs .NET 2.0](http://www.o2sol.com/pdfxplorer/download.htm)

[Official PDF 1.7 reference](http://www.adobe.com/content/dam/Adobe/en/devnet/acrobat/pdfs/pdf_reference_1-7.pdf)

[[GERMAN] How to embed unicode fonts in PDF](http://www.p2501.ch/pdf-howto/typographie/vollzugriff/direkt)

[PDF X/1-a Validator](https://www.pdf-online.com/osa/validate.aspx)

[PDF X/3 technical notes](http://www.pdfxreport.com/lib/exe/fetch.php?media=en:technote_pdfx_checks.pdf)

## Donate

- Bitcoin: 3DkYz32P77Bfv93wPgV66vs1vrUwgStcZU
- Bitcoin Cash: 1QAi8xVB4nRtkaxTAXbzGHmg6nxuuezuYk
- Ethereum: 0xb9960F9970b659056B03CB0241490aDA83A73CEa


## License
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Ffschutt%2Fprintpdf.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2Ffschutt%2Fprintpdf?ref=badge_large)