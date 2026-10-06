//! A `/Span`'s `/ActualText` that its glyphs read as anyway: the same text from the fonts'
//! `/ToUnicode` maps, and no letter-spacing that a reader would take for spaces. Saving with
//! `optimize` drops it, and `optimize_text` drops it from any finished PDF.

use lopdf::{content::Content, dictionary, Document, ObjectId, Stream};
use printpdf::*;

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

/// `text` as a hex `/ActualText` string: UTF-16BE with its byte-order mark
fn utf16(text: &str) -> String {
    let units: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
    format!("<FEFF{units}>")
}

/// A one-page PDF made without printpdf showing `content`. /F1 is an Identity-H font whose
/// /ToUnicode reads code 1 as "H", 2 as "i" and 3 as a space; /F2 is the same without one.
fn pdf(content: &str) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
                1 begincodespacerange <0000> <FFFF> endcodespacerange\n\
                3 beginbfchar <0001> <0048> <0002> <0069> <0003> <0020> endbfchar\n\
                endcmap CMapName currentdict /CMap defineresource pop end end";
    let to_unicode = doc.add_object(Stream::new(dictionary! {}, cmap.as_bytes().to_vec()));
    let font = |doc: &mut Document, to_unicode: Option<ObjectId>| {
        let mut font = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "Test",
            "Encoding" => "Identity-H",
        };
        if let Some(id) = to_unicode {
            font.set("ToUnicode", id);
        }
        doc.add_object(font)
    };
    let (f1, f2) = (font(&mut doc, Some(to_unicode)), font(&mut doc, None));
    let mut stream = Stream::new(dictionary! {}, content.as_bytes().to_vec());
    stream.compress().unwrap();
    let content_id = doc.add_object(stream);
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
        "Contents" => content_id,
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => f1, "F2" => f2 } },
    });
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }.into(),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn page_content(pdf: &[u8]) -> String {
    let doc = Document::load_mem(pdf).unwrap();
    let page = *doc.get_pages().get(&1).unwrap();
    String::from_utf8(doc.get_page_content(page)).unwrap()
}

/// The first page's content after `optimize_text`
fn optimized(content: &str) -> String {
    let bytes = optimize_text(&pdf(content), &mut Vec::new()).expect("optimize_text");
    page_content(&bytes)
}

fn operators(content: &str) -> Vec<String> {
    Content::decode(content.as_bytes())
        .unwrap()
        .operations
        .into_iter()
        .map(|op| format!("{} {:?}", op.operator, op.operands))
        .collect()
}

/// "Hi Hi": kerned inside the words, wider after the space (justified)
const HI_HI: &str = "[<00010002> -20 <0003> -150 <00010002>] TJ";

// --- optimize_text ---

#[test]
fn optimize_text_drops_actual_text_the_glyphs_read_as_anyway() {
    let content = format!(
        "BT /F1 12 Tf /Span <</ActualText {}>> BDC 1 0 0 1 10 10 Tm {HI_HI} EMC ET",
        utf16("Hi Hi")
    );
    let after = optimized(&content);
    assert!(!after.contains("ActualText"), "{after}");
    assert!(!after.contains("BDC") && !after.contains("EMC"), "{after}");
}

#[test]
fn optimize_text_leaves_the_rest_of_the_page_as_it_is() {
    let content = format!(
        "q 0 0 m 100 100 l S Q BT /F1 12 Tf /Span <</ActualText {}>> BDC 1 0 0 1 10 10 Tm {HI_HI} \
         EMC ET",
        utf16("Hi Hi")
    );
    let unspanned: Vec<String> = operators(&content)
        .into_iter()
        .filter(|op| !op.starts_with("BDC") && !op.starts_with("EMC"))
        .collect();
    assert_eq!(operators(&optimized(&content)), unspanned);
}

#[test]
fn optimize_text_keeps_actual_text_the_glyphs_do_not_read_as() {
    let span = |actual: &str, shown: &str| {
        format!(
            "/Span <</ActualText {}>> BDC 1 0 0 1 10 10 Tm {shown} EMC",
            utf16(actual)
        )
    };
    let cases = [
        // letter-spaced: a reader takes the gaps for spaces ("H i")
        (
            "letter-spaced",
            format!("BT /F1 12 Tf {} ET", span("Hi", "[<0001> -456 <0002>] TJ")),
        ),
        (
            "other text",
            format!("BT /F1 12 Tf {} ET", span("Ha", "[<00010002>] TJ")),
        ),
        (
            "a code without text",
            format!("BT /F1 12 Tf {} ET", span("Hi", "[<00010005>] TJ")),
        ),
        (
            "no /ToUnicode",
            format!("BT /F2 12 Tf {} ET", span("Hi", "[<00010002>] TJ")),
        ),
        (
            "character spacing",
            format!("BT /F1 12 Tf 2 Tc {} ET", span("Hi", "[<00010002>] TJ")),
        ),
        (
            "more than /ActualText",
            format!(
                "BT /F1 12 Tf /Span <</ActualText {} /MCID 0>> BDC [<00010002>] TJ EMC ET",
                utf16("Hi")
            ),
        ),
        (
            "nested marked content",
            format!(
                "BT /F1 12 Tf /Span <</ActualText {}>> BDC /X BMC [<00010002>] TJ EMC EMC ET",
                utf16("Hi")
            ),
        ),
    ];
    for (case, content) in cases {
        assert!(
            optimized(&content).contains("ActualText"),
            "{case}: /ActualText dropped"
        );
    }
}

// --- saving ---

/// One line of Roboto, in an /ActualText span as the HTML renderer writes them, with
/// `gap` thousandths of an em between the glyphs
fn saved(text: &str, gap: f32, optimize: bool) -> String {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let mut doc = PdfDocument::new("actual text");
    let font_id = doc.add_font(&font);
    let glyphs = text
        .chars()
        .map(|c| Codepoint {
            gid: font
                .lookup_glyph_index(c as u32)
                .expect("Roboto has the glyph"),
            offset: gap,
            cid: Some(c.to_string()),
        })
        .collect();
    let mut data = vec![0xFE, 0xFF];
    data.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
    let properties = DictItem::Dict {
        map: [(
            "ActualText".to_string(),
            DictItem::String {
                data,
                literal: false,
            },
        )]
        .into(),
    };
    let ops = vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::External(font_id),
            size: Pt(12.0),
        },
        Op::BeginMarkedContentWithProperties {
            tag: "Span".to_string(),
            properties,
        },
        Op::SetTextMatrix {
            matrix: TextMatrix::Raw([1.0, 0.0, 0.0, 1.0, 72.0, 700.0]),
        },
        Op::ShowText {
            items: vec![TextItem::GlyphIds(glyphs)],
        },
        Op::EndMarkedContent,
        Op::EndTextSection,
    ];
    let options = PdfSaveOptions {
        optimize,
        ..Default::default()
    };
    let bytes = doc
        .with_pages(vec![PdfPage::new(Mm(210.0), Mm(297.0), ops)])
        .save(&options, &mut Vec::new());
    page_content(&bytes)
}

#[test]
fn saving_drops_actual_text_the_glyphs_read_as_anyway() {
    let content = saved("Hello world", 0.0, true);
    assert!(!content.contains("ActualText"), "{content}");
    assert!(content.contains("TJ"), "{content}");
}

#[test]
fn saving_keeps_actual_text_of_letter_spaced_glyphs() {
    assert!(saved("Hello", -456.0, true).contains("ActualText"));
}

#[test]
fn saving_without_optimize_keeps_actual_text() {
    assert!(saved("Hello world", 0.0, false).contains("ActualText"));
}
