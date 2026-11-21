#![cfg(feature = "html")]
//! Strings from a PDF end up in the SVG of `PdfPage::to_svg`: tags of marked content, marker ids,
//! layer names, link targets, font names and the names of unknown operators. A hostile string must
//! stay data: the SVG stays well-formed, gets no element or event handler of its own, and a link
//! cannot run script.

use printpdf::*;
use xmlparser::{ElementEnd, Token, Tokenizer};

const HOSTILE: [&str; 6] = [
    "A\" onmouseover=\"alert(1)",
    "x' onclick='alert(1)",
    "</g><script>alert(1)</script>",
    "--><script>alert(1)</script><!--",
    "a&b<c>'d\"e",
    "line\nbreak\ttab",
];

struct Elem {
    name: String,
    attrs: Vec<(String, String)>,
}

impl Elem {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

/// The text of the `<style>` elements and every element with its decoded attributes. Panics if
/// the SVG is not well-formed XML.
fn parse(svg: &str) -> (Vec<Elem>, String) {
    let mut elems: Vec<Elem> = Vec::new();
    let mut style = String::new();
    let mut in_style = false;
    for token in Tokenizer::from(svg) {
        match token.unwrap_or_else(|e| panic!("the SVG is not well-formed XML: {e}\n{svg}")) {
            Token::ElementStart { local, .. } => {
                in_style = local.as_str() == "style";
                elems.push(Elem { name: local.as_str().to_string(), attrs: Vec::new() });
            }
            Token::Attribute { local, value, .. } => {
                if let Some(e) = elems.last_mut() {
                    e.attrs.push((local.as_str().to_string(), unescape(value.as_str())));
                }
            }
            Token::Text { text } if in_style => style.push_str(&unescape(text.as_str())),
            Token::ElementEnd { end: ElementEnd::Close(_, name), .. } if name.as_str() == "style" => {
                in_style = false;
            }
            _ => {}
        }
    }
    (elems, style)
}

fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let decoded = match &tail[1..end] {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// No script, no event handler, and no link that is not `http`, `https`, `mailto`, `tel` or a
/// relative reference.
fn assert_inert(svg: &str) -> Vec<Elem> {
    let (elems, _) = parse(svg);
    for e in &elems {
        assert_ne!(e.name.to_ascii_lowercase(), "script", "a <script> element in\n{svg}");
        for (name, value) in &e.attrs {
            assert!(!name.to_ascii_lowercase().starts_with("on"), "attribute {name}={value:?} in\n{svg}");
            if name == "href" {
                let v = value.trim_start().to_ascii_lowercase();
                assert!(!v.starts_with("javascript:") && !v.starts_with("data:text") && !v.starts_with("vbscript:"), "href {value:?}");
            }
        }
    }
    elems
}

fn render(doc: &PdfDocument, ops: Vec<Op>) -> (String, Vec<PdfWarnMsg>) {
    let page = PdfPage::new(Mm(100.0), Mm(100.0), ops);
    let mut warnings = Vec::new();
    let svg = page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut warnings);
    (svg, warnings)
}

#[test]
fn marked_content_tags_and_marker_ids_stay_attribute_values() {
    let doc = PdfDocument::new("t");
    let mut ops = Vec::new();
    for s in HOSTILE {
        ops.push(Op::BeginMarkedContent { tag: s.to_string() });
        ops.push(Op::Marker { id: s.to_string() });
        ops.push(Op::EndMarkedContent);
    }
    let (svg, _) = render(&doc, ops);
    let elems = assert_inert(&svg);
    let tags: Vec<&str> = elems.iter().filter(|e| e.attr("class") == Some("marked-content")).filter_map(|e| e.attr("data-tag")).collect();
    assert_eq!(tags, HOSTILE, "the tags are read back as they were written");
    let ids: Vec<&str> = elems.iter().filter(|e| e.attr("class") == Some("marker")).filter_map(|e| e.attr("id")).collect();
    assert_eq!(ids, HOSTILE, "the marker ids are read back as they were written");
}

#[test]
fn layer_names_stay_attribute_values() {
    let mut doc = PdfDocument::new("t");
    let mut ops = Vec::new();
    for s in HOSTILE {
        let id = doc.add_layer(&Layer::new(s));
        ops.push(Op::BeginLayer { layer_id: id });
        ops.push(Op::EndLayer);
    }
    // A layer id from the outside (the JSON of the wasm API) is escaped as well.
    ops.push(Op::BeginLayer { layer_id: LayerInternalId("q\" onload=\"x".to_string()) });
    ops.push(Op::EndLayer);
    let (svg, _) = render(&doc, ops);
    let elems = assert_inert(&svg);
    let names: Vec<&str> = elems.iter().filter(|e| e.attr("class") == Some("layer")).filter_map(|e| e.attr("data-name")).collect();
    assert_eq!(names, HOSTILE);
    assert!(elems.iter().any(|e| e.attr("id") == Some("q\" onload=\"x")));
}

#[test]
fn links_that_could_run_script_are_left_out() {
    let doc = PdfDocument::new("t");
    let rect = Rect::from_xywh(Pt(10.0), Pt(10.0), Pt(50.0), Pt(20.0));
    let link = |uri: &str| Op::LinkAnnotation {
        link: LinkAnnotation::new(rect.clone(), Actions::Uri(uri.to_string()), None, None, None),
    };

    let rejected = [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        " \t\u{1}javascript:alert(1)",
        "java\nscript:alert(1)",
        "jav\tascript:alert(1)",
        "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
        "vbscript:msgbox(1)",
        "file:///etc/passwd",
    ];
    let left_out = |w: &Vec<PdfWarnMsg>| w.iter().filter(|w| w.msg.contains("left out of the SVG")).count();
    let (svg, warnings) = render(&doc, rejected.iter().map(|u| link(u)).collect());
    let elems = assert_inert(&svg);
    assert!(elems.iter().all(|e| e.name != "a"), "no link is written:\n{svg}");
    assert_eq!(left_out(&warnings), rejected.len(), "one warning for each link that is left out");

    let kept = [
        "https://example.com/a?b=1&c=\"2\"",
        "http://example.com/",
        "mailto:x@example.com",
        "tel:+4930123456",
        "#top",
        "relative/path.pdf",
        "../up.pdf?x=<1>",
    ];
    let (svg, warnings) = render(&doc, kept.iter().map(|u| link(u)).collect());
    let elems = assert_inert(&svg);
    let hrefs: Vec<&str> = elems.iter().filter(|e| e.name == "a").filter_map(|e| e.attr("href")).collect();
    assert_eq!(hrefs, kept, "the other links are read back as they were written");
    assert_eq!(left_out(&warnings), 0, "{warnings:?}");
}

#[test]
fn the_name_of_an_unknown_operator_cannot_end_the_comment() {
    let doc = PdfDocument::new("t");
    let ops = HOSTILE.iter().map(|s| Op::Unknown { key: s.to_string(), value: Vec::new() }).collect();
    let (svg, _) = render(&doc, ops);
    assert_inert(&svg);
    let comments: Vec<String> = Tokenizer::from(svg.as_str())
        .filter_map(|t| match t {
            Ok(Token::Comment { text, .. }) => Some(text.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(comments.len(), HOSTILE.len(), "one comment for each operator:\n{svg}");
    for c in &comments {
        assert!(c.starts_with(" Unknown PDF operator: ") && !c.contains("--"), "{c:?}");
    }
}

#[test]
fn characters_that_xml_does_not_allow_are_replaced() {
    let doc = PdfDocument::new("t");
    let ops = vec![
        Op::BeginMarkedContent { tag: "bell\u{7}x\u{0}y\u{1F}\u{FFFE}".to_string() },
        Op::EndMarkedContent,
    ];
    let (svg, _) = render(&doc, ops);
    let elems = assert_inert(&svg);
    let tag = elems.iter().find_map(|e| e.attr("data-tag")).expect("the group is written");
    assert_eq!(tag, "bell\u{FFFD}x\u{FFFD}y\u{FFFD}\u{FFFD}");
}

#[test]
fn a_font_name_cannot_leave_the_style_or_the_attribute() {
    let bytes = std::fs::read("examples/assets/fonts/RobotoMedium.ttf").expect("the test font");
    let font = ParsedFont::from_bytes(&bytes, 0, &mut Vec::new()).expect("the test font parses");
    for name in HOSTILE.iter().chain(["F\"; } </style><script>alert(1)</script> {"].iter()) {
        let mut doc = PdfDocument::new("t");
        let id = FontId((*name).to_string());
        doc.resources.fonts.map.insert(id.clone(), printpdf::font::PdfFont::new(font.clone()));
        let ops = vec![
            Op::SetFont { font: PdfFontHandle::External(id), size: Pt(12.0) },
            Op::StartTextSection,
            Op::ShowText { items: vec![TextItem::Text("Hi".to_string())] },
            Op::EndTextSection,
        ];
        let (svg, _) = render(&doc, ops);
        let elems = assert_inert(&svg);
        let (_, style) = parse(&svg);
        let family = elems
            .iter()
            .find(|e| e.name == "text")
            .and_then(|e| e.attr("font-family"))
            .unwrap_or_else(|| panic!("a <text> with a font-family for {name:?}"));
        assert!(
            family.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_')),
            "font-family {family:?} for {name:?}"
        );
        assert!(
            style.contains(&format!("font-family: \"{family}\"")),
            "the @font-face rule and the <text> name the same family {family:?}"
        );
    }
}
