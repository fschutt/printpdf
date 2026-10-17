//! A page reads only the pictures it shows.
//!
//! `PdfDocument::from_html*` copied (and base64-decoded) every entry of the `images` map,
//! and the bridge decoded every one of them, for every render, whether the page's HTML used
//! it or not. Rendering a 1053-page book page by page with its 57 pictures in the map took
//! 22 minutes (1.26 s a page); handing each page only the pictures its `<img src>` names took
//! 3.5 (0.24 s a page). The map is now looked up only for the `src` values the laid-out pages
//! draw, so the rest of it cannot slow a page down, fail it, or end up embedded in it.
//!
//! The decode count itself is checked next to the renderer, in `src/html/mod.rs`
//! (`an_image_the_page_does_not_reference_is_not_decoded`).

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

/// A page with text and no picture.
const TEXT_PAGE: &str = r#"
    <html>
        <body>
            <div>A page without pictures.</div>
        </body>
    </html>
"#;

/// A page that shows `cat.jpg` once (the shape `test_html_img_embeds_through_pipeline` uses).
const CAT_PAGE: &str = r#"
    <html>
        <body>
            <div style="width: 300px; height: 169px;">
                <img src="cat.jpg" style="width: 300px; height: 169px;" />
            </div>
            <div>caption text</div>
        </body>
    </html>
"#;

const NOT_BASE64: &str = "%% not base64 %%";

fn render(
    html: &str,
    images: &BTreeMap<String, Base64OrRaw>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<PdfDocument, String> {
    PdfDocument::from_html(
        html,
        images,
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        warnings,
    )
}

fn mentions(warnings: &[PdfWarnMsg], needle: &str) -> bool {
    warnings.iter().any(|w| w.msg.contains(needle))
}

#[test]
fn an_unreferenced_image_entry_that_is_not_base64_does_not_fail_the_page() {
    let mut images = BTreeMap::new();
    images.insert(
        "plate-12.png".to_string(),
        Base64OrRaw::B64(NOT_BASE64.to_string()),
    );
    let mut warnings = Vec::new();

    let doc = render(TEXT_PAGE, &images, &mut warnings)
        .expect("a picture the page does not show cannot fail it");

    assert!(!doc.pages.is_empty());
    assert!(
        !mentions(&warnings, "plate-12.png"),
        "no warning about a picture the page does not show: {:?}",
        warnings
    );
}

#[test]
fn the_debug_render_does_not_read_unreferenced_image_entries_either() {
    let mut images = BTreeMap::new();
    images.insert(
        "plate-12.png".to_string(),
        Base64OrRaw::B64(NOT_BASE64.to_string()),
    );
    let mut warnings = Vec::new();

    let (doc, _debug) = PdfDocument::from_html_debug(
        TEXT_PAGE,
        &images,
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("a picture the page does not show cannot fail it");

    assert!(!doc.pages.is_empty());
    assert!(!mentions(&warnings, "plate-12.png"), "{:?}", warnings);
}

#[test]
fn a_picture_the_page_shows_that_cannot_be_read_is_a_warning_naming_it() {
    let mut images = BTreeMap::new();
    images.insert(
        "cat.jpg".to_string(),
        Base64OrRaw::B64(NOT_BASE64.to_string()),
    );
    let mut warnings = Vec::new();

    let doc =
        render(CAT_PAGE, &images, &mut warnings).expect("the page renders, without the picture");

    assert!(!doc.pages.is_empty());
    assert!(
        mentions(&warnings, "cat.jpg"),
        "the unreadable picture is reported: {:?}",
        warnings
    );
}

#[cfg(all(feature = "images", feature = "jpeg"))]
#[test]
fn a_picture_no_page_shows_is_not_embedded() {
    let cat: &[u8] = include_bytes!("../examples/assets/img/cat.jpg");
    let mut images = BTreeMap::new();
    images.insert("cat.jpg".to_string(), Base64OrRaw::Raw(cat.to_vec()));
    images.insert("plate-12.jpg".to_string(), Base64OrRaw::Raw(cat.to_vec()));
    let mut warnings = Vec::new();

    let doc = render(CAT_PAGE, &images, &mut warnings).expect("renders");

    let xobjects = &doc.resources.xobjects.map;
    assert!(
        xobjects.contains_key(&XObjectId("HtmlImg_cat_jpg".to_string())),
        "the picture the page shows is embedded"
    );
    assert!(
        !xobjects.contains_key(&XObjectId("HtmlImg_plate_12_jpg".to_string())),
        "the picture it does not show is not"
    );
}
