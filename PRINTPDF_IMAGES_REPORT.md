# PRINTPDF-IMAGES report: decode only the pictures a page shows

Branch `fix/html-images-decoded-per-page` in `/Users/fschutt/Development/printpdf-lazy-images`
(from origin/azul-codegen-api 84dce8c, not pushed). Nothing was compiled here.

## Root cause

`PdfDocument::from_html_with_cache` (and `from_html_debug_with_cache`) copied every
entry of `images` into `XmlRenderOptions::images` (`Raw` cloned, `B64` base64-decoded,
src/lib.rs:462), and `xml_to_pdf_pages` called `bridge::resolve_html_images`, which
decoded every entry (src/html/bridge.rs:108), before looking at what the pages draw.
Every decoded picture was then registered as an XObject, so a page also embedded every
picture of the map. The debug path decoded the whole map twice.

Layout never needed the bytes: azul turns `<img src="x">` into a `NullImage` tagged
with `"x"` and lays it out from CSS / the width/height attributes. So the fix moves the
lookup after layout and restricts it to the `src` values the display lists draw.

## Commits

- 3643f5a RED (tests + a behaviour-neutral, `cfg(test)` decode counter)
- 246c194 fix (+ CHANGELOG)
- 1d805c9 unit test for `referenced_image_srcs`
- ebd2b37 / this commit: progress file, report

## What changed

- `src/html/bridge.rs`
  - `html_image_src(&DisplayListItem)`: the `src` of an `<img>` item (also used by the
    Image arm of the op conversion).
  - `pub fn referenced_image_srcs(&[DisplayList]) -> BTreeSet<String>`: every drawn
    `<img src>`, once.
  - `pub(crate) trait HtmlImageSource` with impls for `BTreeMap<String, Vec<u8>>` and
    `BTreeMap<String, Base64OrRaw>` (base64 decoded only when looked up, same
    `STANDARD.decode` as before).
  - `pub(crate) fn resolve_referenced_html_images(referenced, images, warnings)`:
    decodes those srcs only; a shown src with no entry / bad base64 / undecodable
    bytes becomes a warning `<img src="...">: not drawn: ...`.
  - `decode_html_image` (every decode goes through it) and the `cfg(test)` thread-local
    `HTML_IMAGE_DECODES` counter.
  - `resolve_html_images` (pub) unchanged: still decodes a whole map.
- `src/html/mod.rs`: `xml_to_pdf_pages` / `xml_to_pdf_pages_debug` keep their
  signatures and delegate to new `pub(crate)` `xml_to_pdf_pages_with_images` /
  `xml_to_pdf_pages_debug_with_images(xml, options, images: &dyn HtmlImageSource,
  image_warnings)`; the debug variant also returns the decoded images.
- `src/lib.rs`: `from_html_with_cache` / `from_html_debug_with_cache` no longer copy
  the images map; they pass it to the `_with_images` variants with the caller's
  `warnings`. The debug path registers the images the render decoded instead of
  decoding the whole map again.
- `CHANGELOG.md`: Unreleased entry.
- Tests: `tests/html_images_on_demand.rs` (new), two unit tests in `src/html/mod.rs`,
  one in `src/html/bridge.rs`.

No public signature changed; one public function was added (`referenced_image_srcs`).

Behaviour changes (both deliberate, in the CHANGELOG):
1. An entry no page shows can no longer fail the call (a bad base64 string anywhere in
   the map used to make `from_html*` return `Err("Base64 decode error: ..")`).
2. A picture a page shows that is missing, has bad base64 or cannot be decoded is now a
   warning naming its src in `from_html*`'s `warnings` (missing/undecodable pictures
   were silent; bad base64 used to fail the whole call). `xml_to_pdf_pages` has no
   warnings channel on success, so there the warnings are dropped as before.

CSS `background-image: url(..)` is not covered because the HTML path does not draw it:
it resolves through azul's `ImageCache`, which printpdf passes empty. If that is ever
wired up through `NullImage` tags, `referenced_image_srcs` picks it up.

## Cross-call image cache: not done, on purpose

The only cache object the `_with_cache` API carries is `SharedFontPool`, whose fields
are all `pub`; adding a field breaks every struct literal (`XmlRenderOptions` is the
same). A new type plus a new method would be additive, but: the measured cost is gone
without it (each picture of the book is on about one page), and a cache of decoded
pixels is large (a 2000x3000 RGB plate is 18 MB; the book's 57 would hold about 1 GB).
Within one call each src is decoded once however often it is drawn. If repeated
ornaments ever show up in a profile, the natural follow-up is an opt-in
`HtmlImageCache` (Arc<Mutex<HashMap<(src, content hash), RawImage>>>) passed to a new
`from_html_with_caches`.

Related, not touched: `from_html_with_cache` still clones (and base64-decodes) every
entry of the `fonts` map and re-registers them on every call, even with a font pool.

## Test command (coordinator)

    cd /Users/fschutt/Development/printpdf-lazy-images
    cargo test --features "images jpeg" --test html_images_on_demand
    cargo test --features "images jpeg" --lib -- an_image_the_page_does_not_reference_is_not_decoded the_picture_a_page_shows_is_decoded_once_and_no_other the_srcs_a_render_needs_are_the_img_tags_its_pages_draw_once_each test_html_img_embeds_through_pipeline image_item
    # default features (no image decoding) too:
    cargo test --test html_images_on_demand
    cargo test --lib -- an_image_the_page_does_not_reference_is_not_decoded the_srcs_a_render_needs

On the RED commit 3643f5a: `an_image_the_page_does_not_reference_is_not_decoded` fails
(1 decode), `the_picture_a_page_shows_is_decoded_once_and_no_other` fails (2 decodes),
and in `html_images_on_demand` the two bad-base64 tests and the warning test fail with
`Err("Base64 decode error ..")`; `a_picture_no_page_shows_is_not_embedded` fails
because `HtmlImg_plate_12_jpg` is embedded.

## Spots least sure to compile

1. `src/lib.rs`: `xml_to_pdf_pages_with_images(html, &xml_options, images, warnings)`:
   `&BTreeMap<String, Base64OrRaw>` coerced to `&dyn HtmlImageSource`, and `warnings`
   (a `&mut` parameter) reborrowed there and used again in the `Err` arm.
2. `src/html/bridge.rs`: the `B64` arm `Ok(Cow::Owned(bytes))` relies on unifying with
   the `Raw` arm for `Cow<[u8]>`; the `#[cfg(test)] thread_local! { pub(crate) static .. }`
   read from `src/html/mod.rs` tests as `bridge::HTML_IMAGE_DECODES`.
3. `src/html/mod.rs`: the tuple-dropping closure in `xml_to_pdf_pages_debug`.
4. Runtime, not compile: `a_picture_the_page_shows_that_cannot_be_read_is_a_warning_naming_it`
   runs under default features and needs azul to paint an `<img>` sized only by CSS into
   a `DisplayListItem::Image` (the same pipeline `test_html_img_embeds_through_pipeline`
   checks with images+jpeg).

## How pdfocr can verify the speed-up

1. Point pdfocr's printpdf dependency at this branch (or at azul-codegen-api once it is
   merged there).
2. In `html2pdf/src/main.rs` `render_page`, delete the per-page filter
   (`images.iter().filter(|(name, _)| html.contains(&format!("src=\"{name}\"")))..`) and
   pass the full `images` map to `from_html_with_cache`.
3. Render the 1053-page book: it should stay at about 0.24 s a page (3.5 min), not
   1.26 s (22 min), with the same pictures on the pages as with the filter; any page whose picture is
   missing from the zip now shows up in `warnings` as `<img src="..">: not drawn: ..`.
