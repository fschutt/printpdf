# HTML to PDF

`PdfDocument::from_html` turns an HTML document into a PDF. Layout is done by
[azul-layout](https://crates.io/crates/azul-layout), so the CSS support is the
one of that engine, and pages are cut automatically. Needs the `html` feature
(on by default).

```rust
use printpdf::*;
use std::collections::BTreeMap;

let html = r#"<html><body style="padding: 10mm">
    <h1>Hello</h1>
    <p>Paragraph text that flows onto as many pages as it needs.</p>
</body></html>"#;

let images = BTreeMap::new(); // name -> Base64OrRaw, used by <img src="name">
let fonts = BTreeMap::new();  // name -> Base64OrRaw, used by font-family: name
let options = GeneratePdfOptions::default();
let mut warnings = Vec::new();

let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)?;
let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
```

`from_html` returns `Err(String)` when the input cannot be laid out. Non-fatal
problems are collected in `warnings`.

## Entry points

| Function | Use |
| --- | --- |
| `PdfDocument::from_html` | Render one document. |
| `PdfDocument::from_html_with_cache` | Same, with a shared font pool (`html::build_font_pool`) so many documents do not reload fonts. |
| `PdfDocument::from_html_debug` | Also returns the display lists and PDF operations, for debugging layout. |

## Options

`GeneratePdfOptions`, all fields optional. Fonts are always embedded (and subset when saving, see `PdfSaveOptions::subset_fonts`); image recompression is `PdfSaveOptions::image_optimization`.

| Field | Default | Meaning |
| --- | --- | --- |
| `page_width`, `page_height` | 210, 297 | Page size in mm (A4). |
| `margin_top`, `margin_right`, `margin_bottom`, `margin_left` | 0 | Page margins in mm. |
| `show_page_numbers`, `header_text`, `footer_text`, `skip_first_page` | off | Page numbers, header and footer text. See the note below. |

Page size and margins come from these options. The CSS `@page` rule is not
parsed, except for the `@footnote` area described below. `<header>` and
`<footer>` elements are ordinary blocks in the flow of the page.

Known issue: `show_page_numbers`, `header_text` and `footer_text` are accepted
but currently draw nothing. azul-layout sets this text in the first font
registered in its renderer resources, and printpdf registers none, so the text
is silently dropped. Repeating headers and footers are not available yet.

## Images and fonts

- `images` maps a name to the image bytes. `<img src="name">` draws it. Only
  the images a page draws are decoded. Give the `<img>` a `width` and a `height`,
  as `examples/html_image.rs` does: layout does not read the picture's own
  size, and with only a CSS `width` the image can come out with no height and is
  left out.
- `fonts` maps a family name to font bytes. `font-family: name` selects it.
- Installed system fonts are found through rust-fontconfig. The 14 standard PDF
  fonts are built in and serve as the fallback.

## Page breaks

- `break-before`, `break-after` and `break-inside: avoid` are honoured.
- `widows` and `orphans` apply to paragraphs; a line is never cut in half.
- Table rows are not split across pages, and `<thead>` is repeated on each
  continuation page.
- `float: footnote` moves an element to the foot of the page its call lands on.
  Style the area with `@page { @footnote { ... } }` (`border-top`, `padding`,
  `margin-top`, `max-height`) and the call and marker with `::footnote-call` and
  `::footnote-marker`.

## CSS

The test suite (`tests/html_*.rs`) and the pages in `examples/assets/html/`
exercise: block, inline, flexbox (including `flex-wrap`), `position: relative`
and `absolute`, `overflow`, `columns`, list items, `text-align: justify`,
`hyphens`, `linear-gradient` backgrounds, `border-radius`, `box-shadow` and
`transform`.

`--azul-margin-note: outside` on an absolutely positioned box inside a
multi-column block places it in the margin beside its line. This is an
extension, not CSS.

Properties that the engine does not know are ignored. If something does not
render the way a browser would draw it, [open an issue](https://github.com/fschutt/printpdf/issues)
with the smallest HTML that shows it.

## Examples

`examples/html.rs`, `html_full.rs`, `html_image.rs`, `html_gradient.rs` and
`css_shapes.rs`, run with `cargo run --release --example html --features html`.
An interactive demo is at <https://fschutt.github.io/printpdf>.
