#![cfg(feature = "html")]
//! The page-layout case the regions work is for: a story taken out of the
//! document with `flow-into`, page boxes of their own sizes in pt, and on
//! each page an absolutely placed frame with `flow-from` and a
//! `shape-inside` path with a hole cut out of it (`evenodd`), rendered by
//! one `from_html` call into one PDF, with a report of where the story went
//! and what did not fit.

use std::collections::BTreeMap;

use printpdf::*;

/// `pt` points in millimetres.
fn mm(pt: f32) -> f32 {
    pt * 25.4 / 72.0
}

/// Two hundred words of the story.
fn story() -> String {
    let words = [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
        "juliet",
    ];
    (0..200).map(|i| words[i % words.len()]).collect::<Vec<_>>().join(" ")
}

#[test]
fn a_story_runs_through_shaped_frames_on_pages_of_their_own_sizes() {
    // The first frame is 260 x 120 pt (346.67 x 160 CSS px, the unit a
    // path is in) with a hole in the middle of its upper half; the second
    // is a plain 360 x 60 pt frame on a wider, lower page.
    let html = format!(
        r#"<html><head><style>
        * {{ margin: 0; padding: 0 }}
        body {{ font-size: 10pt; line-height: 12pt }}
        .story {{ flow-into: art7 }}
        .page {{ position: relative; break-after: page; overflow: hidden }}
        #page1 {{ width: 300pt; height: 200pt }}
        #page2 {{ width: 400pt; height: 150pt }}
        .frame {{ position: absolute; left: 20pt; top: 20pt; flow-from: art7 }}
        #frame1 {{ width: 260pt; height: 120pt;
                  shape-inside: path(evenodd, 'M0 0H346.67V160H0Z M130 0H216V80H130Z') }}
        #frame2 {{ width: 360pt; height: 60pt }}
    </style></head><body>
        <div class="story"><p id="para">{}</p></div>
        <div class="page" id="page1"><div class="frame" id="frame1"></div></div>
        <div class="page" id="page2"><div class="frame" id="frame2"></div></div>
    </body></html>"#,
        story()
    );
    let options: GeneratePdfOptions = serde_json::from_str(&format!(
        r#"{{ "page_sizes": [ {{ "width": {}, "height": {} }}, {{ "width": {}, "height": {} }} ],
             "margin_top": 0, "margin_right": 0, "margin_bottom": 0, "margin_left": 0 }}"#,
        mm(300.0),
        mm(200.0),
        mm(400.0),
        mm(150.0)
    ))
    .expect("the options parse");
    let mut warnings = Vec::new();
    let (doc, report) = PdfDocument::from_html_with_report(
        &html,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &options,
        &mut warnings,
        None,
    )
    .expect("the document renders");

    let sizes: Vec<(f32, f32)> = doc
        .pages
        .iter()
        .map(|page| (page.media_box.width.0, page.media_box.height.0))
        .collect();
    assert_eq!(sizes.len(), 2, "a page per page box: {sizes:?}");
    assert!((sizes[0].0 - 300.0).abs() < 0.5 && (sizes[0].1 - 200.0).abs() < 0.5, "{sizes:?}");
    assert!((sizes[1].0 - 400.0).abs() < 0.5 && (sizes[1].1 - 150.0).abs() < 0.5, "{sizes:?}");

    let text = doc.extract_text();
    assert!(text[0].concat().starts_with("alpha"), "the story starts in the first frame: {text:?}");
    assert!(!text[1].concat().trim().is_empty(), "it goes on in the second: {text:?}");

    assert_eq!(report.flows.len(), 1, "{report:#?}");
    let flow = &report.flows[0];
    assert_eq!(flow.name, "art7");
    let regions: Vec<(usize, Option<&str>)> = flow
        .regions
        .iter()
        .map(|region| (region.page, region.element_id.as_deref()))
        .collect();
    assert_eq!(regions, vec![(0, Some("frame1")), (1, Some("frame2"))], "{report:#?}");
    let first = &flow.regions[0];
    assert!(
        (first.rect_pt[0] - 20.0).abs() < 0.5 && (first.rect_pt[1] - 20.0).abs() < 0.5,
        "the first frame stands 20pt in on its page: {first:?}"
    );
    let lines: Vec<usize> = flow
        .regions
        .iter()
        .map(|region| region.paragraphs.iter().map(|p| p.lines).sum())
        .collect();
    assert!(lines.iter().all(|&n| n > 0), "both frames hold lines: {lines:?}");
    assert_eq!(
        flow.regions[1].paragraphs[0].first_line,
        flow.regions[0].paragraphs[0].lines,
        "the second frame goes on with the next line of the paragraph: {report:#?}"
    );
    assert_eq!(
        flow.regions[0].paragraphs[0].element_id.as_deref(),
        Some("para"),
        "{report:#?}"
    );
    let overflow = flow.overflow.as_ref().expect("two hundred words do not fit the two frames");
    assert_eq!(overflow.element_id.as_deref(), Some("para"));
    assert!(overflow.byte_offset.is_some_and(|b| b > 0), "{overflow:?}");
    assert!(overflow.needed_height_pt > 0.0, "{overflow:?}");
}

/// The story through one 260 x 120 pt frame on a 300 x 200 pt page, the
/// frame's `region-fragment` being `rule`: the document and its report.
fn one_frame(rule: &str) -> (PdfDocument, HtmlLayoutReport) {
    let html = format!(
        r#"<html><head><style>
        * {{ margin: 0; padding: 0 }}
        body {{ font-size: 10pt; line-height: 12pt }}
        .story {{ flow-into: art7 }}
        .page {{ position: relative; width: 300pt; height: 200pt; break-after: page }}
        .frame {{ position: absolute; left: 20pt; top: 20pt; width: 260pt; height: 120pt;
                  flow-from: art7; region-fragment: {rule} }}
    </style></head><body>
        <div class="story"><p id="para">{}</p></div>
        <div class="page"><div class="frame" id="frame"></div></div>
    </body></html>"#,
        story()
    );
    let options: GeneratePdfOptions = serde_json::from_str(&format!(
        r#"{{ "page_sizes": [ {{ "width": {}, "height": {} }} ],
             "margin_top": 0, "margin_right": 0, "margin_bottom": 0, "margin_left": 0 }}"#,
        mm(300.0),
        mm(200.0)
    ))
    .expect("the options parse");
    let mut warnings = Vec::new();
    PdfDocument::from_html_with_report(
        &html,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &options,
        &mut warnings,
        None,
    )
    .expect("the document renders")
}

#[test]
fn a_last_frame_of_region_fragment_break_leaves_the_rest_of_the_story_out() {
    let (doc, report) = one_frame("break");
    assert_eq!(doc.pages.len(), 1, "the page box is the only page");
    let overflow = report.flows[0]
        .overflow
        .as_ref()
        .expect("two hundred words do not fit the frame");
    let offset = overflow.byte_offset.expect("the overflow starts in the story's text");
    let words = |text: &str| text.split_whitespace().count();
    let shown = doc.extract_text()[0].join(" ");
    assert_eq!(
        words(&shown),
        words(&story()[..offset]),
        "the page shows the story up to where the overflow starts, and nothing after: {shown:?}"
    );
    assert!(overflow.needed_height_pt > 0.0, "{overflow:?}");

    // The same frame of `region-fragment: auto` lets the rest run on below it.
    let (doc, _) = one_frame("auto");
    let runs_on = doc.extract_text().concat().join(" ");
    assert!(words(&runs_on) > words(&shown), "{runs_on:?}");
}
