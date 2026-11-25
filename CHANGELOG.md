# Changelog

All notable changes to printpdf are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added

- The features `text_layout_hyphenation_all` and `html_hyphenation_all` embed the hyphenation dictionaries of all 82 languages. `text_layout_hyphenation`, and so `html`, hyphenate English (US) only, which makes the binary 2.9 MB smaller (the WebAssembly demo was 13.8 MB).
- `ppdf`, a command-line tool in `ppdf/` (its own crate, not part of the printpdf package). It converts a PDF to JSON and back so the document can be edited with `jq`, prints text, text boxes, SVG pages, fonts and images, and renders HTML to PDF. See `ppdf/README.md`.
- `web/minimal.html`: a single HTML file that makes a PDF in the browser with two wasm calls. It imports the package of the latest release from the demo site, and is deployed with it. Described in `docs/API.md`.
- `docs/HTML.md` documents the HTML and CSS that `from_html` supports. It replaces `SYNTAX.md`, which described an API that no longer exists.
- The library denies `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, indexing and string slicing outside tests (`clippy::unwrap_used` and its relatives, set in `src/lib.rs`). A CI job runs clippy with `--all-features`, `--no-default-features` and `--no-default-features --features text_layout`. On `wasm32-unknown-unknown` a panic traps and cannot be caught, so a failure has to come back as an `Err`, a warning or a skipped item.
- More than 180 regression tests for hostile input in `tests/no_panic_*.rs`: crafted and mutated PDFs, fonts, images, dates, form values, CSS sizes and JSON of the wasm API.
- `GeneratePdfOptions::show_page_numbers`, `header_text`, `footer_text` and `skip_first_page` draw a header, a footer and page numbers on every page (azul-layout 0.0.21; they drew nothing before).
- Form XObjects are part of the document model. A form read from a PDF is an `XObject::Form` with its own `ops`, `/BBox` and resources, and it is written back as a form. `extract_text`, `extract_text_boxes` and `to_svg` read the text and drawing inside forms (PDFs from macOS Quartz draw their whole page in one form; their text was empty before). `FormXObject::new`, `with_matrix`, `with_resources`, `with_group`, `from_content_stream` and `PdfDocument::add_form` create a form in code, for example a watermark that several pages draw.
- Variable fonts from Rust code (feature `text_layout`): `font_variation_axes` lists the axes of a font, `instantiate_variable_font_bytes` creates a static font at chosen axis values, and `PdfDocument::add_variable_font` registers such an instance. TrueType (`gvar`) and CFF2 fonts work; a CFF2 instance is converted to CFF1, which a PDF can embed. The types are `FontVariationTag`, `FontVariationAxis`, `FontVariationSettings` and `VariableFontError`. Based on #278 by NicTanghe.

### Changed

- Pin every GitHub Action to a commit SHA, give workflow tokens no default permissions, and lint the workflows with zizmor. Dependabot keeps the pins up to date (#296).
- Dependabot opens one pull request for all GitHub Actions updates and one for the minor and patch updates of each crate, instead of one per dependency. Major crate updates stay separate.
- Move the web demo sources to `web/`, the documentation to `docs/` and `gen.sh` to `scripts/gen-examples.sh`. `scripts/build-demo.sh` assembles the demo in `_site/`.
- The crates.io package excludes `web/`, `docs/` and `ppdf/`.
- `docs/API.md` now documents all seven functions of the wasm API. It listed five, used wrong option names (`subsetFonts` instead of `subset_fonts`, `pageWidth` instead of `page_width`) and a `title` input that does not exist.
- The README example no longer shows `<head><header>` and `{builtin:pagenum}`, which the layout engine does not implement.
- Write this changelog in one format for every release, and add the releases that had no entry.
- `RawImage::encode_to_bytes`, `apply_dithering` and `resize_to_fit_size` return `Err` when the pixel buffer is not exactly `width * height * channels` samples, and `encode_to_bytes` also for an image of size 0. They used to panic or abort inside the encoder.
- `xml_to_pdf_pages` and `xml_to_pdf_pages_debug` return `Err` when the page, or the area inside the margins, is NaN or outside 1 to 1,000,000 pt.
- A page of a parsed document that cannot be read becomes an empty page with a warning instead of failing the whole document.
- **Breaking:** `FormXObject` has `ops`, `bbox` and its own `resources` (a `PdfResources`) instead of `bytes`; its JSON changes. Forms that a PDF contains are `XObject::Form` and no longer `XObject::External`. A form that cannot be read without loss stays `External`, with an info message.
- `to_svg` multiplies nested `cm` operators. It replaced the transform before.
- `save` warns when a variable font added with `add_font` is embedded at its default instance. Use `add_variable_font` to choose the axes.
- Pages and forms are read and written by the same functions: both use a `PdfResources` as their scope for fonts, images, graphics states and shadings.
- printpdf depends on `glyph-names` and `encoding_rs` directly (both were in the tree through allsorts).
- `Pt` and `Mm` order by `f32::total_cmp` (-0.0 sorts before 0.0, NaN last), so `==` and `cmp` can differ within the 0.0005 tolerance of `==`. `Px` addition and subtraction saturate. `PageState` has two more public fields, `untracked_saves` and `untracked_marked_content`.

### Documentation

- Rewrite the documentation of the public API in plain language for readers who are new to PDF. The crate root has a guide with a first program, and every module explains the part of PDF it covers. All 620 public items that had no documentation now have one, and `#![warn(missing_docs)]` keeps it that way. Remove history, issue numbers, pasted spec text and examples that did not compile; the 34 doctests all run.

### Removed

- **Breaking:** the `font_embedding` and `image_optimization` fields of `GeneratePdfOptions`. `from_html` never read them: fonts are always embedded, and image recompression is `PdfSaveOptions::image_optimization`. JSON that still contains the fields is accepted and the fields are ignored.
- **Breaking:** `render_to_svg` and `render_to_svg_async` are no longer public. Use `PdfPage::to_svg`, `PdfPage::to_svg_async` or `PdfDocument::page_to_svg`; `PdfToSvgOptions` stays.
- Unused files: the working notes of the image decoding change, `subset.ttf`, `build.sh`, a disabled shape test and two example files that were never registered. `CHANGELOG2.md` is merged into this file.

### Fixed

- Saving a document no longer panics when every bookmark points at a page that does not exist, for example after the pages were removed from a parsed document. The outline `/Count` now counts only the bookmarks that are written.
- Reading a CMap (`/ToUnicode` or a Type0 `/Encoding`) no longer panics on a multi-byte character inside a hex token such as `<000é000>`; the token is an error and only costs that font its map.
- A `begincidrange` whose destination runs past `u32::MAX` maps the affected codes to CID 0 (notdef) instead of overflowing.
- Hostile files no longer make the parser loop, run out of memory or overflow the stack. A bookmark `/Next` cycle stops with a warning. References that fan out (a 2 KB file took 1.6 GB) are inlined within a budget of 2 million objects and 256 MiB per document. A page tree whose `/Count` is huge no longer panics with `capacity overflow` (the parser, `optimize_text` and the form reader read the page list page by page). Image and soft-mask sizes that overflow are rejected. A form field value that refers to itself overflowed the stack: values are read to a depth of 8 and 4096 objects.
- Every stream read from a document inflates to at most 256 MiB (CMaps 16 MiB), and `RawImage::decode_from_bytes` sets limits of 512 MiB and 2^20 pixels per side.
- `q`, `BMC` and `BDC` nest at most 256 levels deep (deeper ones are counted and warned about once), and a repeated `W` or `W*` on a path that is already clipped is skipped: 28 KB of them produced 4 million points. The SVG output caps `q` the same way.
- ToUnicode and CID CMaps are limited to 2^20 entries and 1024 codespace ranges, a `usecmap` chain no longer takes quadratic time, and the CFF width scan stops after 10,000 steps per glyph (subroutine fan-out hung it). `/W` runs and the synthetic sfnt of a CFF font no longer overflow.
- A multi-byte first character of a timezone offset, and offset parts that overflow, no longer panic in the date parser.
- The arc of a rounded corner is cut into at most 256 segments (a radius of 1e5 made a million points).
- Simple TrueType fonts without a Unicode character map keep the glyphs of their codes through parse and save. A saved page from macOS Quartz kept none of its text before.
- A simple font with an embedded CFF program (`/Subtype /Type1` with `/FontFile3 /Type1C`, as Quartz, Distiller and ps2pdf write it) survives parse and save. A bare CFF font had no character map, so no character found a glyph on save and the font was dropped, with the text unreadable. The font now gets a map from its glyph names (Adobe Glyph List).
- The text of a simple font follows its `/Encoding` (`StandardEncoding`, `MacRomanEncoding`, `WinAnsiEncoding` and `/Differences`) and the built-in encoding of its CFF program, where it used to read every code as WinAnsi (`ƒtats-Unis` for `États-Unis`). With poppler as the reference, parse, save and parse again keeps 100% of the words of an Apple CFF PDF and about 80% of two TeX PDFs; it was 0%, 7% and 5%.
- The SVG output (`PdfPage::to_svg`, `PdfDocument::page_to_svg`, the wasm `Pdf_PageToSvg`) escapes every string it takes from a PDF: the tags of marked content, marker ids, layer ids and names, link targets, font names and the names of unknown operators. A crafted PDF could add an `onmouseover` attribute or a `<script>` element through them, which runs wherever the SVG is inlined into a page, as the web demo does. A link other than `http`, `https`, `mailto`, `tel` or a relative reference (`javascript:`, `data:`) is left out with a warning. Font names in the `@font-face` rule and the `<text>` elements keep only letters, digits and `.`, `-`, `+`, `_`.

## 0.12.14 - 2026-11-21

### Added

- Add `ParsedFont::from_shared_bytes(bytes: Arc<[u8]>, font_index, warnings)`. It builds the same face as `from_bytes` without copying the font file and decodes glyph outlines on demand.

## 0.12.13 - 2026-11-19

### Added

- `from_html` supports `float: footnote`. A footnote is printed at the bottom of the page where its call appears and continues on the next page if it does not fit.
- Footnote calls and markers come from the `footnote` counter (`content: none` on `::footnote-call` and `::footnote-marker` turns them off). `@page { @footnote { ... } }` styles the area with `border-top`, `padding`, `margin-top` and `max-height`.
- Add `--azul-margin-note: outside` for absolutely positioned boxes inside multi-column blocks. The box is placed outside the block at the height of its line. This is a layout extension, not CSS.

### Changed

- Update azul-css, azul-core and azul-layout to 0.0.20.

### Fixed

- Fix repeated table heads missing their text on continuation pages.

## 0.12.12 - 2026-11-19

### Changed

- Update azul-css, azul-core and azul-layout to 0.0.19.

### Fixed

- Fix `text-align: justify` stretching a line that ends in a forced break such as `<br>`. The line is now set from the start edge (CSS Text 3).

## 0.12.11 - 2026-11-18

### Changed

- `from_html` now converts each page to PDF operations as the layout cuts it (azul-layout `layout_document_paged_streaming`) instead of keeping every page's display list. Output is unchanged. Peak memory for a 227-page document drops from 599 MB to 437 MB (debug build).

## 0.12.10 - 2026-11-18

### Added

- Add the `forms` module: `parse_form_fields` reads a PDF's AcroForm fields, `fill_form` fills or flattens them and stamps drawings onto pages, `fields_svg` draws field values as SVG.
- `text_boxes` words now record their standard-14 font, fill colour and whether they are invisible (`Tr 3`).
- Add `show_placed_glyphs(fonts, size, glyphs, actual_text)` and `PlacedGlyph` to write glyphs where an external text layout placed them. It emits one `TJ` per font stretch, kerned against the `/W` widths from `glyph_pdf_width`, and uses text rise for marks. Rotated text matrices work.
- Add `actual_text_span(text)`, the `/ActualText` span the HTML renderer wraps glyph runs in. `show_placed_glyphs` uses it when given text, so letters that share one glyph copy and search as written. Saving with `optimize` drops the span where the glyphs already read the same.

### Changed

- Update azul-css, azul-core and azul-layout to 0.0.18.

### Fixed

- Fix SVG `<path>` in `from_html`: strokes were missing and fills printed as solid boxes. Strokes are now written as PDF strokes (width, caps, joins, miter limit, dashes) and fills are clipped to the path with its fill rule. A mask that is only an image is still painted unclipped.
- Fix the SVG renderer drawing cubic curves as quadratics and placing shapes under a `cm` transform in the wrong place.
- Fix `re` when reading a PDF: it now appends a closed rectangle to the current path, so `re f` no longer draws nothing and `re W n` no longer paints a box.
- Fix runs in synthetic bold, which are now filled and stroked.
- Fix `/ToUnicode` losing the text of a glyph that was later shown without text, such as a mark in a cluster. An empty text now only fills a glyph that has none.

## 0.12.9 - 2026-11-11

### Added

- Add `optimize_images(pdf, options, warnings)` to re-encode the pictures of a finished PDF in place, and `optimize_text(pdf, warnings)` to drop redundant `/ActualText` spans. Add the `optimize_images` example.
- Add `bridge::referenced_image_srcs`, which lists the `<img src>` values the laid out pages draw.
- Add the `multiple_fonts` example, which switches between faces of a CJK font collection (#283).

### Changed

- Update azul-css, azul-core and azul-layout to 0.0.17 (azul#476). The HTML components use azul-core's `ComponentCodegen`.
- `from_html*`, `xml_to_pdf_pages` and `add_xml_to_document` decode and embed only the pictures the pages draw, once each (#290). Rendering a 1053-page book page by page with 57 pictures drops from 22 to 3.5 minutes. `from_html_debug*` no longer decodes pictures twice. `bridge::resolve_html_images` still decodes a whole map.
- A shown picture with a missing or undecodable entry is now a warning naming its `src` instead of an error. An unused entry can no longer fail the call.
- Write black-and-white pictures with 1 bit per pixel (#290). They are Flate-compressed even when JPEG is requested.
- Write one text object per line instead of one per word (#288). On a 1000-word page the content stream drops from 45 KB to 17 KB with positioned words and from 32 KB to 12 KB with flowing paragraphs.
- `optimize` merges consecutive text objects, skips font and colour settings that are already set, and joins adjacent `/ActualText` spans on one baseline.
- Glyph runs round kerns to whole thousandths of an em and put glyphs without a kern between them in one hex string.

### Fixed

- Fix `from_html`, `from_html_with_cache` and `from_html_debug_with_cache` returning `Ok` with an empty document when the HTML loader could not read the page, for example on U+FFFE or a C0 control character. They now return `Err` with the loader's messages (#294, pdfocr #11).
- Fix `PdfPage::to_svg` re-shaping glyphs in the browser. Shaped runs are now painted from glyph outlines with the PDF's `/W` advances, so Arabic joins, ligatures and Indic conjuncts render as in the PDF (#293).
- Fix CID-keyed CFF fonts read from a PDF losing glyph widths when the first hint operator sits in a subroutine.
- Fix HTML text ignoring colour alpha (#287). `color: transparent` is now invisible text (`3 Tr`) that stays selectable and searchable, and `rgba()` text uses an `ExtGState` alpha. Translucent inline `background-color` is fixed the same way.
- Fix `RawImage::optimize` dithering before scaling down, which turned a flat grey all white. Dithering is now the last step.
- Fix bookmarks on the same page losing their insertion order (#282).
- Fix the bundled base-14 subsets having no space glyph, so every space fell back to a system font. Each of the 14 subsets now has an empty `space` glyph. `!`, `”` and a few others are still missing (#288).
- Fix azul#478: a table that overflows a page now fills every non-final page. The page-break regression tests (#291) run by default.

## 0.12.8 - 2026-09-04

### Changed

- Update rust-fontconfig to 5 (`>=5.0, <6`) and azul to 0.0.16. HTML to PDF font fallback now uses rust-fontconfig 5's tiered fallback chain, with per-script faces and an explicit last resort.
- Use azul 0.0.16 instead of 0.0.15, which pulled `uuid` into every wasm build and broke `--no-default-features --features text_layout` (azul#465).
- `build_font_pool(fonts, Some(&["monospace"]))` and similar calls scan more fonts than before, because generic families now expand to their per-script fallbacks (13 to 27 fonts for `monospace` on macOS). `Some(&[])` still scans nothing.

### Fixed

- The embedded Symbol and ZapfDingbats fonts reported no Unicode coverage, so font fallback never chose them. They now report the 43 and 10 codepoints they map.
- The other embedded standard fonts claimed whole Unicode blocks (Helvetica all 256 of U+0000..U+00FF, against 213 real glyphs), which drew .notdef instead of falling through to a font with the glyph. Coverage now matches each font's cmap.

## 0.12.7 - 2026-08-29

### Changed

- Allow rust-fontconfig 4.6 within the existing `>=4.4.9, <5` range.

### Fixed

- The `optimize` save option now compresses page content and embedded font streams with Flate (#284). `/Length1` keeps the decoded TrueType program size, so readers still load the fonts.
- Remove compiler warnings in reduced-feature builds such as `--no-default-features --features gif` (#285).

## 0.12.6 - 2026-08-17

### Changed

- HTML to PDF now paginates with azul's `layout_document_paged_v2` and the full `BreakPolicy`: `break-inside: avoid`, `widows` and `orphans`, whole line boxes and table rows across pages, and `<thead>` repeated on continuation pages.
- Pages that hit a break constraint now break earlier than before, by at most a third of the page height per break. Documents without such constraints paginate as before.
- Use the published azul 0.0.14 crates instead of a git pin.
- Allow rust-fontconfig 4.5 and later 4.x releases (`>=4.4.9, <5`).

## 0.12.5 - 2026-07-29

### Changed

- Use the published azul-css, azul-core and azul-layout 0.0.13 instead of the azul git branch, and require rust-fontconfig 4.4.9, which can parse the embedded standard PDF fonts.

### Fixed

- HTML rendering resolves `font-family: Helvetica` and the other standard PDF fonts (Arial, Times New Roman, Times, Courier New, Courier, Symbol, Zapf Dingbats and the PDF base-14 names) to the embedded fonts instead of logging an unresolved family and substituting another face.
- The generic families `serif`, `sans-serif` and `monospace` use the embedded fonts only as a fallback behind installed fonts (on wasm they are all that is available). Fonts you pass in win over a built-in of the same name.
- The embedded Helvetica-Bold and Helvetica-BoldOblique subsets lacked the bold bit, so `font-weight: bold` rendered the regular face.

## 0.12.4 - 2026-07-23

### Fixed

- Text selection in Chrome and Edge (PDFium) now spans the whole line for text from the HTML and layout pipelines. The serializer merges per-glyph `Tm` and `TJ` pairs on one baseline into a single kerned `TJ`. Rendering is unchanged.

## 0.12.3 - 2026-07-23

### Fixed

- CID-keyed CFF fonts (such as NotoSansJP) drew wrong glyphs in Chrome (PDFium) and poppler after the #280 change in 0.12.0 (#281). They are now embedded as a bare CFF table (`/FontFile3`, `/Subtype /CIDFontType0C`) instead of the whole OpenType file, which renders correctly everywhere.
- `ParsedFont::from_bytes` reads these bare CFF embeddings back, taking units per em from the FontMatrix and glyph widths from the Type 2 charstrings.

## 0.12.2 - 2026-07-23

### Changed

- Update azul-css, azul-core and azul-layout to 0.0.12 and require rust-fontconfig 4.4.7.
- Remove the `agg-rust-azul` `<1.0.3` pin from the `svg` feature. azul-layout 0.0.11 and later need agg-rust-azul 1.0.3.

### Fixed

- Regular Latin body text in HTML rendering no longer resolves to a CJK font (NotoSansJP). The fix comes with the azul 0.0.12 and rust-fontconfig 4.4.7 updates.

## 0.12.1 - 2026-07-18

### Added

- `CidCMap`, a parser for the code to CID CMaps in a Type0 font's `/Encoding`: variable-width codespace ranges, `cidrange` and `cidchar` tables, `usecmap` for the identity bases and `/WMode`. Identity-H and Identity-V can be built without parsing.
- `PdfPage::extract_text_boxes` and `PdfDocument::extract_text_boxes` return hOCR-style page, line, word and glyph bounding boxes with the decoded text, as serde JSON. Boxes are in pt with a top-left origin, the same space as `to_svg`.

### Changed

- `PdfDocument::parse` decodes Type0 text through the font's `/Encoding` CMap, including embedded CMap streams, variable-width codes and Identity-V. Predefined CJK CMaps and vertical writing mode still warn.
- Update azul-css and azul-core to 0.0.10 and azul-layout to 0.0.11, which no longer reads the clock in a way that aborts on browser wasm.

## 0.12.0 - 2026-07-18 [YANKED]

Yanked from crates.io because the version bump was not warranted. Its changes are included in later
0.12.x releases, so use the latest 0.12.x instead.

### Added

- Warn at save time when text-showing operations appear outside `Op::StartTextSection` and `Op::EndTextSection`, the usual cause of a blank PDF (#254).
- Warn when parsing a Type0 font with a non-Identity-H `/Encoding`, instead of silently scrambling its text.
- `no_auto_scale` on `Op::UseXobject` transforms, so parsed content keeps its own placement.
- WASM API: `Pdf_RegisterFonts` registers the default fonts once instead of resending them on every call, and `Pdf_DecodeImage` decodes an image or signature into the document's `RawImage` JSON.
- Rebuilt wasm demo: tabs, live preview, minimap, warnings drawer, dark mode, a browser HTML reference view, JSON tree editing and save/load of the document, font and image extraction from a parsed PDF, and image optimization on save.

### Changed

- Raw pixel data serializes as base64 (`{"u8b64": ...}`) instead of JSON number arrays, which takes a 1024x1024 RGBA image from about 16 MB to 5.6 MB. The array form still parses.
- The wasm API returns a structured error envelope when the response itself cannot be serialized (it returned an empty string).
- SVG `<text>` without a resolvable font-family now logs a warning instead of being dropped silently.
- Use the published azul-layout 0.0.10, and pin `agg-rust-azul` below 1.0.3 for the `svg` feature, because 1.0.3 broke fresh `svg` builds.

### Fixed

- CID-keyed CFF fonts (such as NotoSansJP) drew wrong glyphs in Acrobat and Preview (#280). The content stream, `/W` widths and `/ToUnicode` are now keyed by the embedded CFF charset's CIDs, as ISO 32000-1 9.7.4.2 requires, on both the subset and the full-embed path.
- Parsing resolves Identity-H codes from CID to glyph through `/CIDToGIDMap` streams and CFF charsets, so PDFs from Adobe, dvipdfmx and similar tools re-render correctly. Legacy files that use code equal to glyph id still decode as before.
- Font dictionaries: the `XXXXXX+` subset tag appears only when the font was subset, `/FontName` matches `/BaseFont`, CIDFontType2 has an explicit `/CIDToGIDMap /Identity`, `.ttc` sources embed only the selected face, and ToUnicode synthesis covers supplementary planes.
- Subset glyph renumbering now uses the subsetter's own mapping, so shaper-substituted glyphs (ligatures, digit alternates) no longer render as tofu or fall back to their first letter.
- HTML: inline-span backgrounds no longer paint over their own text, and custom `font-family` names, including quoted multi-word names, resolve.
- Embedded fonts named `F1` to `F14` are no longer mistaken for the standard 14 fonts on parse or save.
- Text decoding resolves the code width per font at `Tf` instead of guessing from string byte parity. Simple fonts decode per byte through ToUnicode or WinAnsi, Type0 fonts use 2-byte codes with per-glyph Unicode from ToUnicode, synthesized from the font's cmap when the PDF has none.
- Parsing keeps Form XObjects, undecodable images and printpdf's own FlateDecode raw bitmaps (with `/SMask`), and inherits MediaBox through the page tree.
- Parsing round-trips the `'` and `"` operators, link annotations (`/Rect` was misread), layers (`/Properties` names) and `/Shading` resources with `sh` ops.
- Relative `Td` and `TD` operators are accumulated into an absolute text cursor instead of being parsed as absolute positions.
- The ToUnicode CMap parser is now a tokenizer. Single-line CMaps, multi-codepoint `bfchar` targets and surrogate pairs work, a ligature no longer discards the whole CMap, and regenerated CMaps keep multi-character targets.
- `PdfPage::to_svg` renders text again, upright, on the page, in the embedded font and selectable. A re-parsed document whose only font is a subset still falls back in the browser preview, since a subset is not a valid webfont.
- The SVG preview no longer drops text positioned with a text cursor (`Td`) to the page bottom, and `T*` line breaks keep the line-start x.
- SVG conversion carries the svg2pdf page's full `/Resources` into a self-contained Form XObject and keeps the content stream byte for byte. Gradients, tiling patterns, transparency groups and clip paths now render, where `/p0 scn` used to come back as `0 g` (solid black).
- `get_external_font_ids` returns real ids instead of a stub, so page previews receive their fonts.
- Saving a document with images without any image-format feature writes raw FlateDecode pixels instead of panicking.

## 0.11.3 - 2026-07-18

### Changed

- Update azul-css and azul-core to 0.0.10 and azul-layout to 0.0.11, which no longer reads the clock in a way that aborts on browser wasm.

## 0.11.2 - 2026-07-17

### Added

- `Svg::parse_with_fonts` passes caller-supplied fonts to SVG `<text>` conversion. Before, only system fonts were used, and none at all on wasm.
- `build_font_pool_with_memory_fonts` registers pre-patterned `rust_fontconfig` memory fonts, with their metadata intact, for the HTML and text-layout path. `printpdf::html` re-exports `rust_fontconfig` so the pattern types can be built without matching the crate version yourself.

### Changed

- Raise dependency floors that admitted versions that cannot build: `time` >= 0.3.36 and `image` >= 0.25.2. Declare `rust-version = "1.88"`, which lopdf 0.44 already required.
- Use weezl 0.2 so dependency graphs link one LZW coder, bound `rust-fontconfig` to `< 4.5` (4.4.4 linked two copies of allsorts), and drop `time`'s unused serde features and a leftover getrandom 0.3 dependency from wasm builds.
- `unix_timestamp` and `from_unix_timestamp` are now the same calendar arithmetic on every target. The old wasm versions used the browser's local timezone, ignored the stored offset and rejected dates before 1970, and wasi returned 0.

### Fixed

- 0.11.0 and 0.11.1 could not be installed in workspaces whose lockfile resolved `time` below 0.3.48 (#279). printpdf no longer enables lopdf's `time` feature, which only gates `Object <-> time` conversions printpdf never used. To get them, enable that feature from your own manifest.
- `PdfDocument::parse` panicked on a `/CreationDate` containing a non-ASCII byte. Dates are now parsed from bytes.
- A `/ToUnicode` bfrange with `end < start` panicked, and a single `<00000000> <FFFFFFFF> <0041>` line expanded to 2^32 entries (out of memory). Both now return an error that only costs that font its ToUnicode map.
- PDF/A and PDF/X XMP metadata wrote single-digit months (`D:2018-9-19T...`), which veraPDF rejects. Months are now zero-padded.
- The js-sys browser canvas paths are compiled out on wasi targets, whose binaries otherwise carried `window` and canvas imports no wasi runtime can satisfy.

## 0.11.1 - 2026-07-14

### Changed

- Require rust-fontconfig 4.4.5, which uses allsorts 0.17. printpdf previously linked two copies of allsorts (0.17 directly, 0.16 through rust-fontconfig), which gave two incompatible `ParsedFont`, `CmapTarget` and `FontData` types and carried the parser twice. It now links one.
- The empty subset-cmap panic noted in 0.11.0 is fixed in allsorts 0.17. printpdf still embeds the full font when none of the used glyphs are reachable from its cmap.

## 0.11.0 - 2026-07-14

### Changed

- **Breaking:** `LineDashPattern` is now `{ offset: f32, pattern: SmallVec<[f32; 8]> }`, an arbitrary-length dash array, instead of three fixed `Option<i64>` dash and gap pairs (#262, PR #263 by @tower120). Build one with `LineDashPattern::new(offset, &[dash, gap, ...])` or `LineDashPattern::solid()`.
- **Breaking:** without `text_layout`, `FontParseWarning` is a struct instead of a `String` alias, so `ParsedFont::from_bytes` has the same signature with and without the feature (#260).
- **Breaking:** update lopdf 0.39 to 0.44 and allsorts-azul 0.16 to 0.17. Both appear in the public API, so pinning either yourself means bumping in step.
- Remove the `[patch.crates-io]` section. printpdf now builds against the crates it publishes, using azul-css 0.0.9, azul-core 0.0.9, azul-layout 0.0.10 and allsorts-azul 0.17.1. The `[patch]` section is why 0.10.0 shipped with empty embedded fonts (#277).

### Fixed

- The HTML renderer no longer crashes on native targets. It uses azul-layout 0.0.10, because the published 0.0.9 wrote to a hardcoded absolute address and segfaulted.
- The HTML renderer no longer aborts when a subset's cmap would be empty, for example when every glyph is a ligature. printpdf embeds the full font in that case.
- External fonts rendered nothing without `text_layout` (#258). The fallback `ParsedFont::from_bytes` now parses cmap, hmtx, head, hhea and OS/2 with allsorts, and `PdfDocument::parse` can read external Type0 fonts back without the feature.
- FontDescriptor `/ItalicAngle`, `/Flags` and `/StemV` were hardcoded to 0, 32 and 80 for every font. They are now derived from the hhea caret slope and OS/2 `usWeightClass` (#271).
- wasm builds broke on the lopdf update. `getrandom` needs `wasm_js` enabled for each major version in the graph (0.3 and 0.4).

### Security

- Update lopdf to 0.44, which fixes RUSTSEC-2026-0187 (unbounded recursion DoS) (#272, PR #275 by @Zynora-fr).

## 0.10.1 - 2026-07-14

If you are on 0.10.0, upgrade: in that release every external font embeds as an empty `/FontFile2`.

### Added

- `printpdf::font::AzulParsedFont` re-exports `azul_layout::ParsedFont`.

### Changed

- `printpdf::ParsedFont` is now a printpdf type, a newtype over `azul_layout::ParsedFont` with `Deref`, that keeps the source font bytes. Field access and method calls are unchanged; `into_inner()` and `as_azul()` return the azul face.
- `text_layout` now pulls in `rust-fontconfig`.

### Fixed

- External fonts embedded as an empty `/FontFile2` because azul-layout's `ParsedFont::from_bytes` no longer keeps the source bytes. printpdf now retains them itself.
- Subset fonts had no `.notdef` glyph, so the first used glyph was renumbered to glyph 0 and drawn as `.notdef`.
- CFF/OpenType (`OTTO`) fonts were written as `CIDFontType2` with `/FontFile2`. The subtype now follows the font program, and a full `OTTO` font is written to `/FontFile3` as `/Subtype /OpenType`.
- Built-in fonts wrote UTF-8 into a `WinAnsiEncoding` stream, so non-ASCII text did not copy or search correctly (#273). printpdf now owns the WinAnsi table.
- The `'` and `"` text operators ignored the selected font, and text drawn only with them was missing from the PDF.
- A font that cannot be embedded now raises a `PdfWarnMsg::error` and omits `/FontFile` instead of writing an empty one.

## 0.10.0 - 2026-07-06

External fonts embed as an empty `/FontFile2` in this release; use 0.10.1 or later.

### Added

- `PdfDocument::add_shading`, `Op::PaintShading` and the `Shading`, `ShadingGeometry` (axial or radial), `GradientStop` and `ShadingId` types for gradient fills.
- HTML: `<img src>` images are embedded as PDF image XObjects.
- HTML: CSS linear and radial gradients are written as PDF shadings.
- HTML: opacity and alpha colours are applied through ExtGState.
- HTML: text runs are wrapped in `/ActualText` marked content, so copy and paste and accessibility use the real run text.

### Changed

- The HTML layer is ported to azul-css and azul-core 0.0.8 and azul-layout 0.0.9. **Breaking:** the trait-based XML component system is replaced by azul's `ComponentDef` and `ComponentMap`; the `<component>` element is ported to it (#268).
- `PdfDocument::with_pages` now replaces the existing pages, as its documentation says, instead of appending (#224). Use `append_document` to merge documents.
- Without `text_layout`, `ParsedFont::from_bytes` takes the font index as `usize` (#260).
- Requires rust-fontconfig 4.4.4. The `kuchiki` dependency of the `html` feature is removed.

### Fixed

- Font subsetting was disabled by a leftover guard and runs again (#250).
- Subset fonts drew scrambled text because the content stream used original glyph ids.
- Rectangles ignored their `PaintMode` and were invisible (#259).
- FontDescriptor `Ascent`, `Descent` and `FontBBox` are scaled to 1000 units per em, and `CapHeight` comes from the OS/2 table (#271).
- A font was not embedded when a glyph's character could not be looked up (#261).
- `Rect::to_array` rounded to integers, which distorted media, trim and crop boxes (#253).
- Parsing a PDF failed on XObject references that point at stream objects (#216).
- ExtGState `ca` and `CA` were swapped when writing and when parsing. `ca` is the fill alpha and `CA` the stroke alpha.
- HTML: border widths and radii were about 1.33 times too large because CSS pixels were mixed with points, clip regions were not written, and borders on a single side or on opposite sides got round caps.

## 0.9.1 - 2026-02-17

### Added

- `PdfDocument::append_document`, `insert_document` and `page_count` for merging PDFs. Resources and bookmark page numbers are merged and adjusted.
- `PdfDocument::from_html_with_cache`, `from_html_debug` and `from_html_debug_with_cache`, and a public `SharedFontPool` that reuses parsed fonts across render calls.
- `html_multithreaded` feature: multithreaded font loading (not available on WASM).
- HTML: underline, strikethrough and overline are drawn.

### Changed

- HTML: `xml_to_pdf_pages` and its debug variant convert dimensions from PDF points to CSS pixels.
- Depends on azul 0.0.7 and rust-fontconfig 2.0.0 from crates.io. rust-fontconfig's `multithreading` feature is no longer enabled by default; use `html_multithreaded`.
- `Instant::now()` calls are removed from the HTML path so it runs on WASM.

### Fixed

- Without `text_layout`, external fonts did not show in PDF readers because a simple TrueType font dictionary was written for CID-encoded text. A Type0/CID font dictionary is written now (#258).
- HTML: pages that only contained backgrounds and no text were emitted as blank pages and are now skipped.
- The SVG renderer printed `[N glyphs]` for glyph-id text and now outputs the text.

## 0.9.0 - 2026-01-29

### Added

- OpenType (CFF) fonts can be embedded and subsetted (`otf-font` example).
- `PdfDocument::extract_text` and `PdfDocument::to_lopdf_document`, which returns the lopdf `Document`.
- `Op::SetFont`, `Op::ShowText`, `Op::DrawRectangle`, `Op::BeginOptionalContent`, `Op::EndOptionalContent` and `Op::EndMarkedContentWithProperties`, plus `PdfFontHandle` (builtin or external font) and `TextItem::GlyphIds` with the `Codepoint` type (glyph id, offset, source text).
- `Rect` has optional `mode` and `winding_order` fields and `Rect::from_xywh`.
- `PdfFont`, `PrintpdfFontMeta` and `FontEmbeddingMode` keep font embedding metadata next to the parsed font.
- Features `images`, `svg` and `text_layout_hyphenation`.
- Parsing: BMC/BDC marked content, embedded Type1 fonts, the CMYK `k` and `K` operators.

### Changed

- **Breaking:** text operations map 1:1 to PDF. `Op::WriteText`, `WriteTextBuiltinFont`, `WriteCodepoints`, `WriteCodepointsWithKerning`, `SetFontSize` and `SetFontSizeBuiltinFont` are replaced by `Op::SetFont` and `Op::ShowText`.
- **Breaking:** `Op::EndLayer` no longer has a `layer_id`, and `Op::BeginMarkedContentWithProperties` takes one `DictItem` instead of a `Vec`.
- **Breaking:** `PdfResources` fonts are `PdfFont` values, which hold the `ParsedFont` in `parsed_font`.
- **Breaking:** the `image` and `svg2pdf` dependencies are optional. `RawImage::decode_from_bytes` needs the `images` feature, and the `svg` module needs the new `svg` feature, which is not a default.
- HTML to PDF now runs on azul-layout's text3 layout and display list, with new border, pagination, text clipping and hyphenation handling. The `html` feature enables hyphenation.
- Font internals moved to azul-layout (see Removed). With `text_layout`, `ParsedFont`, `OwnedGlyph`, `FontType`, `FontMetrics` (azul's `PdfFontMetrics`) and `PdfFontParseWarning` are azul-layout's types, re-exported by printpdf. `ParsedFont` implements `Serialize`, `Deserialize`, `PartialEq` and `Debug` there and has its own `from_bytes`.
- Without `text_layout`, printpdf provides a stand-in `ParsedFont`, `FontMetrics` and `FontType`.
- Dependencies: lopdf 0.35 to 0.39, `allsorts` 0.16 instead of `allsorts-subset-browser`, and azul 0.0.6 from crates.io.

### Removed

- **Breaking:** `PdfDocument::from_html_async`, `PdfDocument::shape_text` and `PdfResources::shape_text`. Text shaping is available through the new `text_shaping` module (`text_layout` feature).
- The `PrepFont` trait and printpdf's own `GlyphOutline`, `GlyphOutlineOperation`, `OutlineMoveTo`, `OutlineLineTo`, `OutlineQuadTo`, `OutlineCubicTo` and `OwnedGlyphBoundingBox` types. They now come from azul-layout.

### Fixed

- Document info strings (title, author, creator, producer, subject, keywords) are written as UTF-16BE with a BOM.
- The `/ToUnicode` map used CIDs instead of Unicode.
- `Rect::to_polygon` treated `y` as the top edge; it is the bottom edge (#233).
- Bookmark serialization, the PDF version header (back to 1.3) and the default image optimisation quality value.
- Parsing: Type0 `/BaseFont` detection, and `re` no longer fills by default.

## 0.8.2 - 2025-03-20

### Added

- Text shaping: `TextShapingOptions`, `TextAlign`, `ShapedText`, `ShapedLine`, `ShapedWord`, `PdfDocument::shape_text` and `ShapedText::get_ops`.

### Changed

- lopdf is back on 0.35 after 0.8.1 moved to 0.36.

### Removed

- **Breaking:** `TextMatrix::Rotate`. Use `TextMatrix::TranslateRotate` (#152).

### Fixed

- Unicode bookmark titles (#188).
- The date parsing module.
- A glyph width was written even when parsing the glyph failed.

## 0.8.1 - 2025-03-18

### Changed

- Update lopdf to 0.36.

## 0.8.0 - 2025-03-17

The API was rewritten; code written against 0.7.x needs porting.

### Added

- Reading PDFs: `PdfDocument::parse` and `parse_async`, and `PdfPage::extract_text`.
- `PdfDocument::from_html` and `from_html_async`: HTML to PDF through azul-layout (`html` feature, experimental).
- `PdfDocument::page_to_svg`, `page_to_svg_async` and `PdfPage::to_svg` render a page to SVG.
- `PdfDocument::save`, `save_async` and `save_writer`, which writes to any `io::Write` without building the bytes in memory first.
- `PdfSaveOptions` with `optimize`, `subset_fonts`, `secure` (skip unknown operations) and `image_optimization` (`ImageOptimizationOptions`).
- A WASM API and web demo. The document types derive serde `Serialize` and `Deserialize`.
- CMap generation and parsing for `/ToUnicode` and font decoding.

### Changed

- **Breaking:** a new API built on `PdfDocument::new(title)`, `PdfPage::new(width, height, ops)` and `Op` values. `PdfDocumentReference`, `PdfLayerReference` and the other reference types are gone.
- **Breaking:** features `embedded_images`, `font_subsetting`, `annotations`, `logging`, `less-optimization`, `svg`, `jpeg_rayon` and `dxt` are removed. The default feature is `html` instead of `js-sys`.
- Dependencies: lopdf 0.31 to 0.35, image 0.24 to 0.25, svg2pdf 0.8 to 0.13. `owned_ttf_parser` is dropped and `allsorts-subset-browser` 0.16 is added.

## 0.7.0 - 2024-01-24

### Added

- `PdfLayerReference::add_rect`. A `Rect` has a paint mode and winding order (`Rect::with_mode`), so it can also clip.
- `WindingOrder::get_fill_stroke_op`.
- Images with an alpha channel (RGBA8) are written with a soft mask.
- `js-sys` feature, enabled by default, so WASM builds can opt out of js-sys.

### Changed

- **Breaking:** `PolygonMode` is renamed `PaintMode`, and `PaintMode` and `WindingOrder` moved to the `printpdf::path` module.
- **Breaking:** `Rect` has new public fields `mode` and `winding`.
- Dependencies: owned_ttf_parser 0.19, svg2pdf 0.8, usvg 0.35, pdf-writer 0.9.

### Fixed

- PDF timestamps used the month name instead of its number and ignored the date's UTC offset.

## 0.6.0 - 2023-09-13

### Added

- Link annotations (hyperlinks): `LinkAnnotation` and `PdfLayerReference::add_link_annotation`.
- `PdfPageReference::extend_with` adds raw lopdf dictionary entries to a page, for example annotations.
- Font subsetting from the glyphs actually used, behind the `font_subsetting` feature. It can be turned on per font with `add_external_font_with_subsetting` and `add_external_font_data_with_subsetting`.
- `Ord` and `Eq` for `Pt` and `Mm`, and `Clone` for `Svg`.

### Changed

- **Breaking:** `Mm`, `Pt`, colour components and the numeric parameters of the drawing API (font sizes, line widths, spacing) use `f32` instead of `f64`.
- **Breaking:** `Line` no longer has `has_fill`, `has_stroke`, `is_clipping_path` or their setters, and `PdfLayerReference::add_shape` is replaced by `add_line`. Filled and clipping shapes, with holes, use the new `Polygon` (`rings`, `PolygonMode`, `WindingOrder`) and `add_polygon`.
- Edition 2021. lopdf 0.27 to 0.31 and time 0.2 to 0.3.25, which drops the stdweb dependency.

### Fixed

- Embedded fonts: not all glyphs were on the width table, subsetting failed for fonts without `€`, and unused fonts are omitted.
- Subsetting errors are handled instead of panicking.
- The WASM build failed because `js_sys::Date` was moved more than once.

## 0.5.3 - 2022-08-20

### Added

- `PdfMetadata`, the document info dictionary and XMP metadata have `author`, `creator`, `producer`, `keywords`, `subject` and `identifier`. `PdfDocumentReference` has matching `with_author`, `with_creator`, `with_producer`, `with_keywords`, `with_subject` and `with_identifier` methods.

### Changed

- `PdfDocumentReference::with_title` returns `Self` instead of `()`.
- image 0.24.3 (#124).

## 0.5.2 - 2022-03-10

### Changed

- Enable all features on docs.rs.

## 0.5.1 - 2022-02-13

### Changed

- **Breaking:** `SvgParseError::Svg2PdfConversionError` carries the underlying error message as a `String`, so `SvgParseError` is no longer `Copy`. `SvgParseError` implements `Display` and `Error`.
- **Breaking:** `SvgTransform` translations and `SvgRotation` centre coordinates are `Pt` instead of `Mm` and `Px`.

### Fixed

- `Svg::parse` used the width as both width and height of the bounding box, and failed on fractional bounding boxes, which are now rounded up.

## 0.5.0 - 2021-12-18

### Added

- `Svg` type to add SVG files to the PDF and place them on the page.

### Changed

- `embedded_images` is no longer a default feature.
- The default PDF conformance no longer embeds an entire ICC colour profile, which saves file size.

## 0.4.1 - 2021-09-19

### Added

- `PdfDocument::save_to_bytes()` saves the document to a `Vec<u8>` (#101).

## 0.4.0 - 2021-06-24

### Changed

- Re-release of 0.3.4 to fix semver breakage. No other changes.

## 0.3.4 - 2020-12-16

### Added

- Bookmarks and clipping paths.
- `PdfDocumentReference::with_creation_date` (#71).
- `FontData` trait for custom font backends, with `add_external_font_data` and `ExternalFont::with_font_data`.

### Changed

- **Breaking:** the default `PdfConformance` no longer requires XMP metadata or an embedded ICC profile.
- **Breaking:** text state operators take `f64` instead of `i64`, for fine-grained character and word spacing.
- Fonts are stored in a `BTreeMap`, so output is reproducible.

### Removed

- The `Error::description` implementations.

## 0.3.3 - 2020-10-17

### Added

- `write_codepoints` and `write_positioned_codepoints` on `PdfLayerReference`.
- `less-optimization` feature, which turns off some compression and optimization in release builds.
- `wasm32-unknown-unknown` support: a `date` module provides `OffsetDateTime` through js-sys.
- `Div` for `Mm`, `Pt` and `Px`, and `From` conversions for `Pt` and `Mm` instead of `Into`.
- JPEG images are written with the `DCTDecode` filter.

### Changed

- `PdfDocument::new` accepts different string types for the title and the initial layer name.
- lopdf 0.24 to 0.26.

### Fixed

- Built-in fonts encoded text as UTF-8 instead of `WinAnsiEncoding`, which broke non-ASCII characters (#30).

## 0.3.2 - 2020-02-28

### Fixed

- The width list in the font descriptor.

## 0.3.1 - 2020-02-26

### Changed

- Update dependencies.

### Fixed

- Fonts on iOS and macOS.

## 0.3.0 - 2020-01-03

### Added

- `PdfDocument::empty`.

### Changed

- Upgrade `rusttype` to 0.8.2. This breaks semver for non-`edition = "2019"` compilers, hence the new version.
- Upgrade `time` to 0.2.1.

## 0.2.12 - 2019-09-12

### Changed

- Upgrade `image` to 0.22.

## 0.2.11 - 2019-08-14

### Changed

- Update `lopdf` (fixes #27).

## 0.2.10 - 2018-10-06

### Added

- `ColorType::Palette` for indexed colours.

### Changed

- **Breaking:** creating an image from a `DynamicImage` can no longer fail, so it does not return a `Result`.
- Upgrade `image` to 0.20.

## 0.2.9 - 2018-09-19

### Added

- Function to create images from an `image::DynamicImage`.

### Changed

- **Breaking:** the `image` crate has the lesser-used formats ico, tga, hdr, dxt and webp disabled by default, so image decoding can fail at runtime unless you re-enable them. Enable `jpeg_rayon` for faster JPEG decoding.
- The `image` dependency is optional.
- Upgrade `lopdf` to 0.17, which drops the large `chrono` dependency. `cargo build --no-default-features` has 33 dependencies and a debug build takes roughly 20 seconds.

### Removed

- The unnecessary `rand` dependency.

## 0.2.8 - 2018-08-03

### Fixed

- The Firefox PDF viewer works correctly, after a fix to the embedded TTF font type.

## 0.2.7 - 2018-08-03

### Fixed

- Incorrect generation of character map files when embedding fonts (https://github.com/fschutt/printpdf/issues/20#issuecomment-409988462).

## 0.2.6 - 2018-04-29

### Added

- `rusttype::Error` is publicly re-exported, so applications can handle it.

### Changed

- **Breaking:** the `error-chain` generated errors are replaced by simpler error enums.
- Update `image` to 0.19.0 and `rand` to 0.5.0.

### Removed

- `FontError`, which was unused.

## 0.2.5 - 2018-03-22

### Added

- `Pt` and `Mm` can be multiplied and divided by `f64`.
- `utils::calculate_points_for_rect` and `utils::calculate_points_for_circle`.

### Fixed

- Word spacing in 0.2.3 and 0.2.4: the horizontal advance width was not taken into account.

## 0.2.4 - 2018-02-13

### Changed

- Update dependencies, because `rusttype` was yanked and 0.2.3 no longer builds.

## 0.2.3 - 2018-02-12

### Changed

- Use `rusttype`; freetype is no longer required, and printpdf builds on Windows without further setup.
- **Breaking:** units are typed. Use `Pt(f64)`, `Mm(f64)` or `Px(f64)` instead of `f64`.
- **Breaking:** `Line` has no `new()` function. Build it with `Line { has_stroke: true, ... }`.

### Removed

- **Breaking:** the `mm_to_pt!` and `pt_to_mm!` macros. Convert between the unit types instead.

### Fixed

- An ugly character-spacing hack.

## 0.2.2 - 2018-02-05

### Changed

- `PdfDocument::save()` needs only `T: Write` instead of `T: Write + Seek`.

### Removed

- SVG functionality, which did not work.
- The `num` dependency.

## 0.2.1 - 2017-11-27

### Added

- `PdfDocument` implements `Clone`, so one document can be written to several outputs.
- `CustomPdfConformance` can disable the automatic ICC profile embedding (see `examples/no_icc.rs`).

### Changed

- **Breaking:** `document.save()` needs a `BufWriter`.
- `set_outline_thickness` accepts floating-point units.
