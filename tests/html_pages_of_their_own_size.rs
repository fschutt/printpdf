#![cfg(feature = "html")]
//! An HTML document of page boxes renders as one PDF with a page per box,
//! each page of its own size: the sizes come in `page_sizes`, a page that
//! holds only a background is a page too, and a story flows through the
//! frames of its pages (CSS Regions).
//!
//! `from_html` laid every page out at one size, dropped the pages it found
//! nothing but backgrounds on (the PDF's page numbers then drifted from the
//! document's), and had no way to run one story through frames on several
//! pages.

use std::collections::BTreeMap;

use printpdf::*;

/// The options of `json`: unknown fields are ignored, so the test reads the
/// same on a printpdf without `page_sizes`.
fn options(json: &str) -> GeneratePdfOptions {
    serde_json::from_str(json).expect("the options parse")
}

/// `pt` points in millimetres.
fn mm(pt: f32) -> f32 {
    pt * 25.4 / 72.0
}

/// The page sizes of `doc`, in points.
fn page_sizes(doc: &PdfDocument) -> Vec<(f32, f32)> {
    doc.pages
        .iter()
        .map(|page| (page.media_box.width.0, page.media_box.height.0))
        .collect()
}

fn render(html: &str, options: &GeneratePdfOptions) -> PdfDocument {
    let mut warnings = Vec::new();
    PdfDocument::from_html(html, &BTreeMap::new(), &BTreeMap::new(), options, &mut warnings)
        .expect("the document renders")
}

#[test]
fn each_page_box_is_a_page_of_its_own_size() {
    let html = r#"<html><head><style>
        * { margin: 0; padding: 0 }
        body { font-size: 12pt }
        .page { position: relative; break-after: page; overflow: hidden }
        #p1 { width: 300pt; height: 400pt }
        #p2 { width: 500pt; height: 300pt }
    </style></head><body>
        <div class="page" id="p1">one</div><div class="page" id="p2">two</div>
    </body></html>"#;
    let options = options(&format!(
        r#"{{ "page_sizes": [ {{ "width": {}, "height": {} }}, {{ "width": {}, "height": {} }} ],
             "margin_top": 0, "margin_right": 0, "margin_bottom": 0, "margin_left": 0 }}"#,
        mm(300.0),
        mm(400.0),
        mm(500.0),
        mm(300.0)
    ));
    let doc = render(html, &options);
    let sizes = page_sizes(&doc);
    assert_eq!(sizes.len(), 2, "a page per page box: {sizes:?}");
    for (got, want) in sizes.iter().zip([(300.0, 400.0), (500.0, 300.0)]) {
        assert!(
            (got.0 - want.0).abs() < 0.5 && (got.1 - want.1).abs() < 0.5,
            "pages {sizes:?}, wanted 300 x 400 and 500 x 300 pt"
        );
    }
    let text = doc.extract_text();
    assert!(text[0].concat().contains("one"), "{text:?}");
    assert!(text[1].concat().contains("two"), "{text:?}");
}

#[test]
fn a_page_with_only_a_background_is_a_page() {
    let html = r#"<html><head><style>
        * { margin: 0; padding: 0 }
        body { font-size: 12pt }
        .page { position: relative; break-after: page; width: 300pt; height: 200pt }
        .blank { background: #ccddee }
    </style></head><body>
        <div class="page">one</div><div class="page blank"></div><div class="page">three</div>
    </body></html>"#;
    let options = options(&format!(
        r#"{{ "page_width": {}, "page_height": {}, "margin_top": 0, "margin_right": 0,
             "margin_bottom": 0, "margin_left": 0 }}"#,
        mm(300.0),
        mm(200.0)
    ));
    let doc = render(html, &options);
    let text = doc.extract_text();
    assert_eq!(text.len(), 3, "the blank page keeps its place: {text:?}");
    assert!(text[2].concat().contains("three"), "{text:?}");
}

#[test]
fn a_story_flows_through_the_frames_of_its_pages() {
    let html = r#"<html><head><style>
        * { margin: 0; padding: 0 }
        body { font-size: 12pt; line-height: 15pt }
        .story { flow-into: art7 }
        .page { position: relative; break-after: page; width: 300pt; height: 200pt }
        .frame { position: absolute; left: 20pt; top: 20pt; width: 260pt; height: 30pt;
                 flow-from: art7 }
    </style></head><body>
        <div class="story">Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo
        lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray
        yankee zulu alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima</div>
        <div class="page"><div class="frame"></div></div>
        <div class="page"><div class="frame"></div></div>
    </body></html>"#;
    let options = options(&format!(
        r#"{{ "page_width": {}, "page_height": {}, "margin_top": 0, "margin_right": 0,
             "margin_bottom": 0, "margin_left": 0 }}"#,
        mm(300.0),
        mm(200.0)
    ));
    let doc = render(html, &options);
    let text = doc.extract_text();
    assert_eq!(text.len(), 2, "two pages: {text:?}");
    let first = text[0].concat();
    let second = text[1].concat();
    assert!(first.starts_with("Alpha"), "the story starts in the first page's frame: {text:?}");
    assert!(
        !second.trim().is_empty(),
        "the story goes on in the second page's frame: {text:?}"
    );
}
