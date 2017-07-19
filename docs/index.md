`printpdf` is a PDF library for creating printable (PDF-X conform) PDF documents. 
It should make the process of generating PDF easier

[Crates.io](https://crates.io/crates/printpdf) | [Documentation](https://docs.rs/printpdf)

```
[dependencies]
printpdf = "0.1.0"
```
You also need to install the FreeType library as printpdf links to it (statically).

## Features

Currently, printpdf can only write documents, not read them.

- Page generation
- Layers (Illustrator like layers)
- Graphics (lines, shapes, bezier curves)
- Images (currently BMP only or generate your own images)
- Embedded fonts (TTF and OTF) with Unicode support
- Advanced graphics - overprint control, blending modes, etc.
- Advanced typography - character scaling, character spacing, superscript, subscript, outlining, etc.
- PDF layers (you should be able to open the PDF in Illustrator and have the layers appear)

## Roadmap (planned)

- Clipping
- Aligning / layouting text
- Open Prepress Interface
- Halftoning images, Gradients, Patterns
- SVG / instantiated content
- More font support
- Forms, annotations
- Bookmarks / Table of contents
- Conformance / error checking for various PDF standards
- Embedded Javascript
- Reading PDF
- Completion of printpdf wiki

## News

### Release 0.1

