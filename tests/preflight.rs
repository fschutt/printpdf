//! The preflight on documents that printpdf writes, and `PdfDocument::preflight`.
//!
//! The rules themselves are tested with PDFs built by `lopdf` in `src/preflight/tests.rs`, so
//! that the writer of printpdf does not grade itself. This file only asks what the preflight
//! says about the writer.

use printpdf::preflight::{Finding, PreflightProfile, Severity};
use printpdf::*;

const FONT: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

fn text_ops(font: PdfFontHandle) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(10.0), Mm(280.0)),
        },
        Op::SetFont {
            font,
            size: Pt(16.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text("Hello".to_string())],
        },
        Op::EndTextSection,
    ]
}

/// A document with default content and one embedded font.
fn document(conformance: PdfConformance) -> PdfDocument {
    let mut doc = PdfDocument::new("Preflight test");
    doc.metadata.info.conformance = conformance;
    let font = ParsedFont::from_bytes(FONT, 0, &mut Vec::new()).expect("font");
    let id = doc.add_font(&font);
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        text_ops(PdfFontHandle::External(id)),
    ));
    doc
}

fn errors(findings: &[Finding]) -> Vec<(&'static str, &str)> {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| (f.rule, f.message.as_str()))
        .collect()
}

#[test]
fn x3_2002_document_with_an_embedded_font_has_no_errors() {
    let doc = document(PdfConformance::X3_2002_PDF_1_3);
    let report = doc.preflight(PreflightProfile::PdfX3_2002);
    assert_eq!(
        errors(&report.findings),
        Vec::new(),
        "{:#?}",
        report.findings
    );
    assert_eq!(report.summary.pages_checked, 1);
}

#[test]
fn x1a_2001_document_with_an_embedded_font_has_no_errors() {
    let doc = document(PdfConformance::X1A_2001_PDF_1_3);
    let report = doc.preflight(PreflightProfile::PdfX1a_2001);
    // PDF/X-1a:2001 requires an output intent; printpdf writes one for it.
    assert_eq!(
        errors(&report.findings),
        Vec::new(),
        "{:#?}",
        report.findings
    );
}

#[test]
fn the_default_document_has_no_general_errors() {
    for conformance in [
        PdfConformance::default(),
        PdfConformance::X3_2002_PDF_1_3,
        PdfConformance::X1A_2001_PDF_1_3,
        PdfConformance::A1B_2005_PDF_1_4,
        PdfConformance::A2B_2011_PDF_1_7,
    ] {
        let report = document(conformance.clone()).preflight(PreflightProfile::General);
        assert_eq!(
            errors(&report.findings),
            Vec::new(),
            "{conformance:?}: {:#?}",
            report.findings
        );
    }
}

#[test]
fn a_builtin_font_is_flagged_by_the_standards_and_not_by_general() {
    // With the default conformance printpdf writes the standard font as a name only. A foreign
    // file looks the same, and the standards report it.
    let mut doc = PdfDocument::new("Builtin");
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        text_ops(PdfFontHandle::Builtin(BuiltinFont::Helvetica)),
    ));
    let x3 = doc.preflight(PreflightProfile::PdfX3_2002);
    assert!(errors(&x3.findings)
        .iter()
        .any(|(r, _)| *r == "font-not-embedded"));
    let general = doc.preflight(PreflightProfile::General);
    assert!(!general.has_errors());
    assert!(general
        .findings
        .iter()
        .any(|f| f.rule == "font-not-embedded" && f.severity == Severity::Info));
}

#[test]
fn a_standard_that_needs_embedded_fonts_embeds_the_builtin_ones() {
    // Before, `PdfConformance::X3_2002_PDF_1_3` and the PDF/A variants wrote Helvetica as a name
    // only, and the preflight reported `font-not-embedded`.
    for (conformance, profile) in [
        (PdfConformance::X3_2002_PDF_1_3, PreflightProfile::PdfX3_2002),
        (PdfConformance::A1B_2005_PDF_1_4, PreflightProfile::PdfA1b),
        (PdfConformance::A2B_2011_PDF_1_7, PreflightProfile::PdfA2b),
    ] {
        let mut doc = PdfDocument::new("Builtin");
        doc.metadata.info.conformance = conformance.clone();
        doc.pages.push(PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            text_ops(PdfFontHandle::Builtin(BuiltinFont::Helvetica)),
        ));
        let report = doc.preflight(profile);
        assert!(
            report.findings.iter().all(|f| f.rule != "font-not-embedded"),
            "{conformance:?}: {:#?}",
            report.findings
        );
    }
}

#[test]
fn pdfa_conformance_writes_what_pdfa_needs() {
    // `PdfConformance::A1B_2005_PDF_1_4` used to write the identifier in the Info dictionary and
    // an output intent of the PDF/X kind, but no XMP metadata. Now the XMP packet carries the
    // PDF/A identification and the output intent is a PDF/A one. `tests/pdfa_output.rs` has the
    // rest.
    for (conformance, profile) in [
        (PdfConformance::A1B_2005_PDF_1_4, PreflightProfile::PdfA1b),
        (PdfConformance::A2B_2011_PDF_1_7, PreflightProfile::PdfA2b),
    ] {
        let report = document(conformance.clone()).preflight(profile);
        let rules: Vec<_> = report.findings.iter().map(|f| f.rule).collect();
        for gone in ["xmp-missing", "pdfa-id-missing", "output-intent-subtype", "font-not-embedded"] {
            assert!(!rules.contains(&gone), "{conformance:?}: {gone} in {rules:?}");
        }
        assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
    }
}

#[test]
fn a_file_without_the_pdfa_parts_is_still_reported() {
    // The default conformance writes no XMP metadata and no output intent, so the PDF/A
    // profiles report both. This is what the rules are for: files that did not come from a
    // PDF/A variant.
    let report = document(PdfConformance::default()).preflight(PreflightProfile::PdfA1b);
    let rules: Vec<_> = report.findings.iter().map(|f| f.rule).collect();
    assert!(rules.contains(&"xmp-missing"), "{rules:?}");
}

#[test]
fn writer_warnings_become_findings() {
    let mut doc = PdfDocument::new("Warnings");
    // Text outside of a text section is reported by the save.
    let ops = vec![Op::ShowText {
        items: vec![TextItem::Text("x".to_string())],
    }];
    doc.pages.push(PdfPage::new(Mm(50.0), Mm(50.0), ops));
    let mut warnings = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings);
    assert!(!warnings.is_empty(), "the save is expected to warn");

    let report = doc.preflight(PreflightProfile::General);
    assert!(
        report.findings.iter().any(|f| f.rule == "writer-warning"),
        "{:#?}",
        report.findings
    );
}

// Two problems of the writer that the preflight found in files printpdf wrote. Both are fixed;
// these tests keep them fixed.

#[test]
fn x3_2002_output_intent_names_its_profile_with_the_right_key() {
    // The key of an output intent's profile is /DestOutputProfile.
    let report = document(PdfConformance::X3_2002_PDF_1_3).preflight(PreflightProfile::PdfX3_2002);
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.rule != "output-intent-profile-missing"),
        "{:#?}",
        report.findings
    );
}

#[test]
fn xmp_metadata_text_is_escaped() {
    // A title with an ampersand and an angle bracket must not make the XMP packet invalid XML.
    let mut doc = document(PdfConformance::X3_2002_PDF_1_3);
    doc.metadata.info.document_title = "Fish & Chips <b>".to_string();
    doc.metadata.info.creator = "A \"B\" & C".to_string();
    let report = doc.preflight(PreflightProfile::PdfX3_2002);
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.rule != "xmp-not-well-formed"),
        "{:#?}",
        report.findings
    );
}

#[test]
fn the_report_is_json() {
    let report = document(PdfConformance::X3_2002_PDF_1_3).preflight(PreflightProfile::PdfX3_2002);
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["profile"], "pdfx-3");
    assert!(json["findings"].is_array());
    assert!(json["summary"]["errors"].is_u64());
    assert!(json["notChecked"].is_array());
    assert!(json["summary"]["pagesChecked"].is_u64());
}
