//! Preflight: check a PDF file for problems and report them with a severity, a location and a
//! stable rule id.
//!
//! This is not a certification. A report with no errors means that none of the rules listed
//! below found a problem. It does not mean that the file conforms to PDF/A or PDF/X, and it
//! does not replace veraPDF, Acrobat Preflight or the checks of your printer. The rules are
//! the ones that can be decided from the file with a small, readable amount of code; what is
//! not checked is listed below and is also part of every [`PreflightReport`].
//!
//! # Entry points
//!
//! - [`preflight_bytes`] checks the bytes of a file. Most rules need the raw structure of the
//!   file (the header, the cross-reference table, the trailer, object streams, resource
//!   dictionaries), so this is the main function. It reads the file with `lopdf`, which repairs
//!   many kinds of damage while loading; the repair itself is reported as a finding.
//! - [`PdfDocument::preflight`] checks a document in memory. It saves the document with the
//!   default [`PdfSaveOptions`] (or the ones you pass to
//!   [`PdfDocument::preflight_with_options`]), checks the bytes it wrote, and adds the warnings
//!   of the save as findings. It checks the file the document would become, not the model.
//!
//! ```
//! use printpdf::*;
//! use printpdf::preflight::{preflight_bytes, PreflightProfile};
//!
//! let mut doc = PdfDocument::new("Preflight example");
//! let page = PdfPage::new(Mm(210.0), Mm(297.0), Vec::new());
//! let bytes = doc.with_pages(vec![page]).save(&PdfSaveOptions::default(), &mut Vec::new());
//!
//! let report = preflight_bytes(&bytes, PreflightProfile::General);
//! assert!(!report.has_errors());
//!
//! // The same file is not a PDF/A-1b file: it has no XMP metadata.
//! let report = preflight_bytes(&bytes, PreflightProfile::PdfA1b);
//! assert!(report.findings.iter().any(|f| f.rule == "xmp-missing"));
//! ```
//!
//! A document that is saved with `PdfConformance::A1B_2005_PDF_1_4`, `A2B_2011_PDF_1_7` or
//! `X1A_2001_PDF_1_3` and uses only what that standard allows gives a report without errors for
//! the profile of the same name (`tests/pdfa_output.rs`). The rules `xmp-missing`,
//! `pdfa-id-missing`, `output-intent-subtype` and `font-not-embedded` then fire on files from
//! other writers, or on a document with the default conformance. If a document uses something
//! that the standard forbids, the save warns, and `PdfDocument::preflight` shows the warning as
//! a `writer-warning` finding next to the rule of the file.
//!
//! # Profiles
//!
//! A [`PreflightProfile`] chooses which rules apply and how severe a finding is.
//!
//! - `General`: structure and resource integrity. It reports what an arbitrary reader may
//!   trip over and does not require any standard.
//! - `PdfA1b` and `PdfA2b`: the General rules plus a subset of the requirements of ISO 19005-1
//!   and ISO 19005-2 that can be decided from the file.
//! - `PdfX1a_2001` and `PdfX3_2002`: the General rules plus a subset of the requirements of
//!   ISO 15930-1:2001 and ISO 15930-3:2002.
//!
//! # Rules
//!
//! Each rule has a stable id, which is the `rule` of a [`Finding`]. [`RULES`] lists all of them
//! with a description. The severity of a rule depends on the profile; `A` below means the
//! PDF/A profiles, `X` the PDF/X profiles.
//!
//! File and cross-reference structure:
//!
//! - `file-unreadable`: the file could not be loaded at all.
//! - `header-missing`, `header-not-at-start` (an error for A), `header-version-invalid`,
//!   `header-binary-comment-missing` (A), `pdf-version-too-new` (A, X): the `%PDF-1.x` line.
//! - `eof-marker-missing`, `data-after-eof` (A), `startxref-missing`: the end of the file.
//! - `xref-reconstructed`, `xref-offset-corrected`, `xref-entry-bad-offset`: the
//!   cross-reference table could not be read as written or points at the wrong place.
//! - `trailer-root-missing`, `catalog-type-wrong`, `trailer-size-wrong`, `trailer-id-missing`
//!   (an error for A, a warning for X), `encrypted` (an error for A and X, information for
//!   General).
//! - `object-stream-forbidden`, `xref-stream-forbidden` (PDF/A-1 and X): PDF 1.5 file structure.
//! - `dangling-reference`: a reference to an object that does not exist.
//!
//! Streams:
//!
//! - `stream-length-missing`, `stream-length-mismatch`: `/Length` against the bytes in the file.
//! - `stream-filter-unknown`, `stream-decode-failed`.
//! - `stream-external-file` (A): a stream whose data is in another file.
//! - `lzw-filter` (A), `filter-not-in-base-version` (PDF/A-1 and X): filters that the standard
//!   does not allow (LZW, JPEG 2000, JBIG2, depending on the standard).
//!
//! Pages and resources:
//!
//! - `pages-tree-missing`, `page-tree-cycle`, `page-tree-node-invalid`, `page-count-mismatch`,
//!   `no-pages`.
//! - `mediabox-missing`, `mediabox-invalid`, `page-box-invalid`, `page-size-huge`,
//!   `page-size-tiny`, `page-box-outside-mediabox` (an error for X), `page-rotate-invalid`.
//! - `trimbox-missing` (X), `trimbox-and-artbox` (X).
//! - `contents-invalid`, `content-stream-unreadable`.
//! - `resource-not-found`: a content stream uses a font, XObject, graphics state, colour space,
//!   pattern or shading by a name that its resources do not have.
//! - `resource-dangling`: a resource entry refers to an object that does not exist.
//! - `resources-malformed`: a resource category is not a dictionary.
//! - `graphics-state-unbalanced`, `text-object-unbalanced`, `text-outside-text-object`:
//!   `q`/`Q` and `BT`/`ET` pairs, and text-showing operators outside `BT` ... `ET`.
//! - `form-bbox-missing`, `xobject-subtype-invalid`, `postscript-xobject` (A, X).
//! - `image-dict-incomplete`, `image-no-colour-space`.
//!
//! Colour:
//!
//! - `colour-space-malformed`, `colour-space-unknown`, `icc-profile-invalid`: for example an
//!   `/ICCBased` array without a profile stream, a `/Separation` array of the wrong length or
//!   an `/Indexed` space with a bad base (the kind of error behind #211).
//! - `device-colour-without-output-intent` (A): `DeviceRGB`, `DeviceCMYK` or `DeviceGray` is
//!   used and neither a matching output intent nor a `Default...` colour space exists.
//! - `rgb-colour-in-pdfx1a`, `colour-space-not-allowed` (PDF/X-1a): RGB, and calibrated, Lab
//!   or ICC-based colour.
//!
//! Fonts:
//!
//! - `font-not-embedded` (an error for A and X; a warning for General, and information for the
//!   14 standard fonts), `font-descriptor-missing`, `font-program-invalid`,
//!   `font-subtype-invalid`.
//! - `font-no-tounicode` (General only): the text of the font cannot be extracted reliably.
//!
//! Transparency and optional content:
//!
//! - `transparency-smask`, `transparency-alpha`, `transparency-blend-mode`,
//!   `transparency-group` (PDF/A-1 and X).
//! - `optional-content` (PDF/A-1 and X): layers.
//!
//! Document information, metadata and output intent:
//!
//! - `info-title-missing`, `info-date-missing`, `trapped-missing`, `pdfx-version-missing`,
//!   `pdfx-version-mismatch` (X).
//! - `output-intent-missing` (X), `output-intent-invalid`, `output-intent-subtype` (A),
//!   `output-intent-profile-missing` (A, and a warning for PDF/X-3).
//! - `xmp-missing` (A), `xmp-stream-invalid`, `xmp-not-well-formed`, `pdfa-id-missing` (A),
//!   `pdfa-id-mismatch` (A).
//!
//! Active and external content:
//!
//! - `javascript`, `launch-action`, `named-action-not-allowed` (A), `remote-goto-action`,
//!   `uri-javascript`.
//! - `embedded-file`.
//! - `3d-content`, `video-content`, `audio-content`, `multimedia-content`. For the PDF/A and
//!   PDF/X profiles the severity follows
//!   [`PdfConformance::is_3d_content_allowed`](crate::PdfConformance::is_3d_content_allowed)
//!   and its siblings. The General profile reports them as information.
//!
//! Notes:
//!
//! - `check-limit-reached` (information): a bound on the work was reached, so the report is
//!   incomplete.
//! - `content-not-checked` (information): a part of the file was skipped.
//! - `writer-warning`: only from [`PdfDocument::preflight`], a warning of the save.
//!
//! # What is not checked
//!
//! The list is [`NOT_CHECKED`] and appears in every report. In short: glyph coverage and widths
//! of fonts and the validity of font programs beyond their first bytes; the content of ICC
//! profiles beyond their header; image data; the consistency of the Info dictionary and the
//! XMP metadata; PDF/A level A structure (tagged PDF); annotation flags and appearance streams;
//! form fields; character procedures of Type 3 fonts; the implementation limits of the
//! standards; overprint, halftone and transfer function rules; trapping; digital signatures;
//! and everything that depends on rendering the page.
//!
//! Some simplifications can hide a problem. A `Default...` colour space counts for the whole
//! document, although it applies to the resources it is in. A font counts as used when a text
//! operator shows text with it, and text render mode 3 (invisible) is not treated specially.
//! Fonts, images and forms are checked only when a content stream draws them, so a font that
//! is listed in the resources and never used is not reported (every reachable stream is still
//! checked for its length and its filters).
//!
//! # Limits on the work
//!
//! A hostile file must not make the check run forever or use unbounded memory. These bounds
//! apply, and reaching one adds a `check-limit-reached` finding:
//!
//! - at most 100,000 pages, a page tree 64 levels deep, and 2,000,000 visited objects;
//! - forms, patterns and colour spaces nested at most 12 levels deep;
//! - 128 MiB of decoded content streams in all, and 128 MiB of decoded data of the other
//!   streams, which are decoded to see whether their filters work; one stream decodes to at
//!   most 64 MiB;
//! - 20 million content stream operators in all, and no parsing of a content stream that is
//!   larger than 16 MiB once decoded (`content-not-checked`);
//! - 500,000 cross-reference entries that are compared with the bytes of the file;
//! - 1,000 findings; further ones are counted in [`PreflightSummary::suppressed`].
//!
//! The check does not panic. A file that cannot be read gives a `file-unreadable` finding.
//!
//! # Clause numbers
//!
//! [`Finding::standard_reference`] names a clause (for example `"ISO 19005-1 6.3.4"`) only
//! where the clause number is known to be right, which is the case for some rules of PDF/A-1b.
//! The other profiles have none. The PDF/X rules follow the requirements of the 2001 and 2002
//! editions as common preflight tools apply them.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde_derive::Serialize;

use crate::{PdfConformance, PdfDocument, PdfParseErrorSeverity, PdfSaveOptions};

mod colour;
mod content;
mod file;
mod structure;
mod util;
pub(crate) mod xmp;

#[cfg(test)]
mod tests;

/// The most pages that are visited.
pub(crate) const MAX_PAGES: usize = 100_000;
/// The deepest page tree that is followed.
pub(crate) const MAX_TREE_DEPTH: usize = 64;
/// The deepest nesting of forms, patterns and colour spaces that is followed.
pub(crate) const MAX_NESTING: usize = 12;
/// The most objects that are visited.
pub(crate) const MAX_OBJECTS: usize = 2_000_000;
/// The most findings kept in a report.
pub(crate) const MAX_FINDINGS: usize = 1_000;
/// Decoded bytes of all content streams together.
pub(crate) const MAX_CONTENT_BYTES: usize = 128 * 1024 * 1024;
/// Decoded bytes of the streams that are decoded only to see that they decode.
pub(crate) const MAX_VALIDATION_BYTES: usize = 128 * 1024 * 1024;
/// The largest output of one stream.
pub(crate) const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;
/// Content stream operators in all.
pub(crate) const MAX_OPERATORS: usize = 20_000_000;

/// How serious a [`Finding`] is. Serialized in lower case.
///
/// The order is `Info < Warning < Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A fact that is good to know. It does not make the file wrong.
    Info,
    /// The file may work but is unusual, or breaks a rule that most readers tolerate.
    Warning,
    /// The file breaks a rule of the format or of the chosen standard.
    Error,
}

impl Severity {
    /// The lower case name: `"info"`, `"warning"` or `"error"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// Which set of rules to apply. See the module documentation.
///
/// Serialized as the short names that `ppdf check --profile` takes: `general`, `pdfa-1b`,
/// `pdfa-2b`, `pdfx-1a` and `pdfx-3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[allow(non_camel_case_types)]
pub enum PreflightProfile {
    /// Structure and resource integrity, without a standard.
    #[serde(rename = "general")]
    General,
    /// PDF/A-1b (ISO 19005-1, level B).
    #[serde(rename = "pdfa-1b")]
    PdfA1b,
    /// PDF/A-2b (ISO 19005-2, level B).
    #[serde(rename = "pdfa-2b")]
    PdfA2b,
    /// PDF/X-1a:2001 (ISO 15930-1).
    #[serde(rename = "pdfx-1a")]
    PdfX1a_2001,
    /// PDF/X-3:2002 (ISO 15930-3).
    #[serde(rename = "pdfx-3")]
    PdfX3_2002,
}

impl PreflightProfile {
    /// Every profile.
    pub const ALL: [PreflightProfile; 5] = [
        PreflightProfile::General,
        PreflightProfile::PdfA1b,
        PreflightProfile::PdfA2b,
        PreflightProfile::PdfX1a_2001,
        PreflightProfile::PdfX3_2002,
    ];

    /// The short name: `general`, `pdfa-1b`, `pdfa-2b`, `pdfx-1a` or `pdfx-3`.
    pub fn name(self) -> &'static str {
        match self {
            PreflightProfile::General => "general",
            PreflightProfile::PdfA1b => "pdfa-1b",
            PreflightProfile::PdfA2b => "pdfa-2b",
            PreflightProfile::PdfX1a_2001 => "pdfx-1a",
            PreflightProfile::PdfX3_2002 => "pdfx-3",
        }
    }

    /// The name of the standard, such as `"PDF/X-3:2002"`.
    pub fn label(self) -> &'static str {
        match self {
            PreflightProfile::General => "General",
            PreflightProfile::PdfA1b => "PDF/A-1b",
            PreflightProfile::PdfA2b => "PDF/A-2b",
            PreflightProfile::PdfX1a_2001 => "PDF/X-1a:2001",
            PreflightProfile::PdfX3_2002 => "PDF/X-3:2002",
        }
    }

    /// The profile for a short [`name`](Self::name), or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }

    /// The profile that matches the standard a document claims, or `None` for a standard that
    /// has no profile.
    ///
    /// ```
    /// use printpdf::{PdfConformance, preflight::PreflightProfile};
    /// assert_eq!(
    ///     PreflightProfile::for_conformance(&PdfConformance::X3_2002_PDF_1_3),
    ///     Some(PreflightProfile::PdfX3_2002)
    /// );
    /// ```
    pub fn for_conformance(c: &PdfConformance) -> Option<Self> {
        match c {
            PdfConformance::A1B_2005_PDF_1_4 | PdfConformance::A1A_2005_PDF_1_4 => {
                Some(PreflightProfile::PdfA1b)
            }
            PdfConformance::A2_2011_PDF_1_7
            | PdfConformance::A2A_2011_PDF_1_7
            | PdfConformance::A2B_2011_PDF_1_7
            | PdfConformance::A2U_2011_PDF_1_7 => Some(PreflightProfile::PdfA2b),
            PdfConformance::X1A_2001_PDF_1_3 => Some(PreflightProfile::PdfX1a_2001),
            PdfConformance::X3_2002_PDF_1_3 => Some(PreflightProfile::PdfX3_2002),
            _ => None,
        }
    }

    /// The conformance whose `is_*_allowed` methods decide about 3D, audio, video and
    /// JavaScript for this profile.
    fn conformance(self) -> PdfConformance {
        match self {
            PreflightProfile::General => PdfConformance::default(),
            PreflightProfile::PdfA1b => PdfConformance::A1B_2005_PDF_1_4,
            PreflightProfile::PdfA2b => PdfConformance::A2B_2011_PDF_1_7,
            PreflightProfile::PdfX1a_2001 => PdfConformance::X1A_2001_PDF_1_3,
            PreflightProfile::PdfX3_2002 => PdfConformance::X3_2002_PDF_1_3,
        }
    }

    pub(crate) fn is_pdfa(self) -> bool {
        matches!(self, PreflightProfile::PdfA1b | PreflightProfile::PdfA2b)
    }

    pub(crate) fn is_pdfx(self) -> bool {
        matches!(
            self,
            PreflightProfile::PdfX1a_2001 | PreflightProfile::PdfX3_2002
        )
    }

    /// A standard, not the General profile.
    pub(crate) fn is_standard(self) -> bool {
        self != PreflightProfile::General
    }

    /// Profiles that forbid transparency: PDF/A-1 and both PDF/X profiles.
    pub(crate) fn forbids_transparency(self) -> bool {
        matches!(
            self,
            PreflightProfile::PdfA1b | PreflightProfile::PdfX1a_2001 | PreflightProfile::PdfX3_2002
        )
    }

    /// Profiles that forbid optional content (layers).
    ///
    /// The profiles for which [`PdfConformance::is_layering_allowed`] is `false`.
    pub(crate) fn forbids_layers(self) -> bool {
        self.forbids_transparency()
    }

    /// Profiles that forbid object streams and cross-reference streams (PDF 1.5).
    pub(crate) fn forbids_compressed_structure(self) -> bool {
        self.forbids_transparency()
    }

    /// The newest header version the standard is built on.
    pub(crate) fn base_version(self) -> Option<(u32, u32)> {
        match self {
            PreflightProfile::General => None,
            PreflightProfile::PdfA1b => Some((1, 4)),
            PreflightProfile::PdfA2b => Some((1, 7)),
            PreflightProfile::PdfX1a_2001 | PreflightProfile::PdfX3_2002 => Some((1, 3)),
        }
    }

    pub(crate) fn allows_3d(self) -> bool {
        self.conformance().is_3d_content_allowed()
    }

    pub(crate) fn allows_video(self) -> bool {
        self.conformance().is_video_content_allowed()
    }

    pub(crate) fn allows_audio(self) -> bool {
        self.conformance().is_audio_content_allowed()
    }

    pub(crate) fn allows_javascript(self) -> bool {
        self.conformance().is_javascript_content_allowed()
    }

    /// The text of `/GTS_PDFXVersion` for the PDF/X profiles.
    pub(crate) fn pdfx_identifier(self) -> Option<&'static str> {
        match self {
            PreflightProfile::PdfX1a_2001 => Some("PDF/X-1a:2001"),
            PreflightProfile::PdfX3_2002 => Some("PDF/X-3:2002"),
            _ => None,
        }
    }
}

/// One problem that the preflight found.
///
/// Serialized to JSON with camelCase field names; `page`, `object` and `standardReference` are
/// left out when they are `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// Stable id of the rule, such as `"font-not-embedded"`. The ids are listed in [`RULES`].
    pub rule: &'static str,
    /// What was found, in a sentence.
    pub message: String,
    /// The page the finding is about, counted from 1 as viewers do. `None` for findings about
    /// the file or the document as a whole.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    /// The number and generation of the object the finding is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<(u32, u16)>,
    /// The clause of the standard, for example `"ISO 19005-1 6.3.4"`. Only set where the
    /// clause number is certain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard_reference: Option<&'static str>,
}

/// Counts and bounds of a [`PreflightReport`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightSummary {
    /// Number of findings with [`Severity::Error`].
    pub errors: usize,
    /// Number of findings with [`Severity::Warning`].
    pub warnings: usize,
    /// Number of findings with [`Severity::Info`].
    pub infos: usize,
    /// Findings that were left out of the report because it holds at most 1,000. They are not
    /// included in the counts above.
    pub suppressed: usize,
    /// Pages that were visited.
    pub pages_checked: usize,
    /// Objects that were visited.
    pub objects_visited: usize,
    /// The bounds on the work that were reached; empty when the check was complete.
    pub limits_reached: Vec<&'static str>,
}

/// The result of a preflight.
///
/// [`has_errors`](Self::has_errors) false means that no rule found an error, not that the file
/// conforms to the standard of the profile. See the module documentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    /// The profile that was applied.
    pub profile: PreflightProfile,
    /// What was found, in the order the checks ran: the header and the end of the file, the
    /// cross-reference table, the document, every reachable object, then the pages. Findings
    /// that are reported once for the whole file, with a count of the others, come last.
    pub findings: Vec<Finding>,
    /// Counts.
    pub summary: PreflightSummary,
    /// What the preflight does not check. The same list as [`NOT_CHECKED`].
    pub not_checked: Vec<&'static str>,
}

impl PreflightReport {
    /// Whether at least one finding is an [`Severity::Error`].
    pub fn has_errors(&self) -> bool {
        self.summary.errors > 0
    }

    /// The findings of one rule.
    pub fn findings_for_rule<'a>(&'a self, rule: &'a str) -> impl Iterator<Item = &'a Finding> {
        self.findings.iter().filter(move |f| f.rule == rule)
    }

    /// Adds a finding after the check and updates the counts.
    fn add_finding(&mut self, f: Finding) {
        if self.findings.len() >= MAX_FINDINGS {
            self.summary.suppressed = self.summary.suppressed.saturating_add(1);
            return;
        }
        match f.severity {
            Severity::Error => self.summary.errors = self.summary.errors.saturating_add(1),
            Severity::Warning => self.summary.warnings = self.summary.warnings.saturating_add(1),
            Severity::Info => self.summary.infos = self.summary.infos.saturating_add(1),
        }
        self.findings.push(f);
    }
}

/// What the preflight does not check. See the module documentation.
pub const NOT_CHECKED: &[&str] = &[
    "glyph coverage, widths and the validity of font programs beyond their first bytes",
    "the content of ICC profiles beyond the header, and whether a profile fits the output condition",
    "image data, image resolution and the consistency of image dictionaries beyond the colour space",
    "consistency between the Info dictionary and the XMP metadata",
    "XMP metadata schemas beyond the PDF/A identification, and extension schemas",
    "logical structure and tagged PDF (PDF/A level A, PDF/UA)",
    "annotation flags, annotation appearance streams and form fields",
    "the content streams of Type 3 character procedures",
    "implementation limits of the standards (nesting depth, string and array sizes)",
    "overprint, halftone, transfer function and rendering intent rules",
    "trapping, OPI and printer marks",
    "digital signatures and the contents of embedded files",
    "anything that depends on rendering the page",
];

/// A rule of the preflight: its id and what it looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInfo {
    /// The id, as in [`Finding::rule`].
    pub id: &'static str,
    /// What the rule looks for.
    pub description: &'static str,
}

macro_rules! rules {
    ($($id:literal => $description:literal,)*) => {
        &[$(RuleInfo { id: $id, description: $description }),*]
    };
}

/// Every rule that the preflight can report, with its id and a description.
pub const RULES: &[RuleInfo] = rules! {
    "file-unreadable" => "the file could not be loaded",
    "header-missing" => "no %PDF- header in the first 1024 bytes",
    "header-not-at-start" => "bytes before the %PDF- header",
    "header-version-invalid" => "the header version is not 1.0 to 1.7 or 2.0",
    "header-binary-comment-missing" => "no comment of four bytes above 127 after the header",
    "pdf-version-too-new" => "the header version is newer than the standard allows",
    "eof-marker-missing" => "no %%EOF marker at the end",
    "data-after-eof" => "data after the last %%EOF marker",
    "startxref-missing" => "no startxref keyword at the end",
    "xref-reconstructed" => "the cross-reference table was unreadable and was rebuilt by scanning",
    "xref-offset-corrected" => "a startxref or /Prev offset did not point at a cross-reference section",
    "xref-entry-bad-offset" => "a cross-reference entry does not point at its object",
    "trailer-root-missing" => "the trailer has no usable /Root",
    "catalog-type-wrong" => "the catalog has no /Type /Catalog",
    "trailer-size-wrong" => "the trailer /Size is smaller than the highest object number",
    "trailer-id-missing" => "the trailer has no /ID",
    "encrypted" => "the file is encrypted",
    "object-stream-forbidden" => "object streams in a standard based on PDF 1.4 or older",
    "xref-stream-forbidden" => "a cross-reference stream in a standard based on PDF 1.4 or older",
    "dangling-reference" => "a reference to an object that does not exist",
    "stream-length-missing" => "a stream without /Length",
    "stream-length-mismatch" => "/Length differs from the bytes between stream and endstream",
    "stream-filter-unknown" => "a /Filter name that PDF does not define",
    "stream-decode-failed" => "a stream whose filters fail to decode",
    "stream-external-file" => "a stream whose data is in an external file",
    "lzw-filter" => "LZWDecode, which PDF/A does not allow",
    "filter-not-in-base-version" => "JPXDecode, JBIG2Decode or BrotliDecode where the base version of the standard has none",
    "pages-tree-missing" => "the catalog has no usable /Pages",
    "page-tree-cycle" => "the page tree contains itself",
    "page-tree-node-invalid" => "a page tree node is not a dictionary or does not exist",
    "page-count-mismatch" => "/Count of the root differs from the number of pages",
    "no-pages" => "the document has no pages",
    "mediabox-missing" => "a page has no /MediaBox, own or inherited",
    "mediabox-invalid" => "a /MediaBox that is not four numbers or has no area",
    "page-box-invalid" => "a CropBox, BleedBox, TrimBox or ArtBox that is not four numbers",
    "page-size-huge" => "a page larger than 14400 units, the limit of Acrobat",
    "page-size-tiny" => "a page smaller than 3 units, the limit of Acrobat",
    "page-box-outside-mediabox" => "a page box that is not inside the MediaBox",
    "page-rotate-invalid" => "/Rotate that is not a multiple of 90",
    "trimbox-missing" => "PDF/X page with neither TrimBox nor ArtBox",
    "trimbox-and-artbox" => "PDF/X page with both TrimBox and ArtBox",
    "contents-invalid" => "/Contents is not a stream or an array of streams",
    "content-stream-unreadable" => "a content stream that cannot be decoded or parsed",
    "resource-not-found" => "a content stream uses a resource name that the resources do not have",
    "resource-dangling" => "a resource entry refers to an object that does not exist",
    "resources-malformed" => "a resource category that is not a dictionary",
    "graphics-state-unbalanced" => "q and Q do not match in a content stream",
    "text-object-unbalanced" => "BT and ET do not match in a content stream",
    "text-outside-text-object" => "a text-showing operator outside BT ... ET",
    "form-bbox-missing" => "a form XObject without /BBox",
    "xobject-subtype-invalid" => "an XObject that is not an image, form or PostScript object",
    "postscript-xobject" => "a PostScript XObject",
    "image-dict-incomplete" => "an image without /Width, /Height or /BitsPerComponent",
    "image-no-colour-space" => "an image that is not a mask and has no /ColorSpace",
    "colour-space-malformed" => "a colour space array or dictionary of the wrong shape",
    "colour-space-unknown" => "a colour space name or family that PDF does not define",
    "icc-profile-invalid" => "an ICC profile stream with a bad /N or without the ICC signature",
    "device-colour-without-output-intent" => "device colour without a matching output intent or default colour space (PDF/A)",
    "rgb-colour-in-pdfx1a" => "DeviceRGB in PDF/X-1a",
    "colour-space-not-allowed" => "calibrated, Lab or ICC-based colour in PDF/X-1a",
    "font-not-embedded" => "a used font whose program is not in the file",
    "font-descriptor-missing" => "a font that is not one of the 14 standard fonts and has no /FontDescriptor",
    "font-program-invalid" => "a font file that is missing, empty or has a wrong signature",
    "font-subtype-invalid" => "a font dictionary without a known /Subtype",
    "font-no-tounicode" => "a font whose text cannot be mapped to Unicode",
    "transparency-smask" => "a soft mask",
    "transparency-alpha" => "a constant opacity below 1",
    "transparency-blend-mode" => "a blend mode other than Normal",
    "transparency-group" => "a transparency group",
    "optional-content" => "optional content (layers)",
    "info-title-missing" => "PDF/X without /Title in the Info dictionary",
    "info-date-missing" => "PDF/X without /CreationDate or /ModDate",
    "trapped-missing" => "PDF/X without /Trapped True or False",
    "pdfx-version-missing" => "PDF/X without /GTS_PDFXVersion",
    "pdfx-version-mismatch" => "/GTS_PDFXVersion names another standard",
    "output-intent-missing" => "PDF/X without an output intent",
    "output-intent-invalid" => "an output intent that is not a dictionary or lacks required keys",
    "output-intent-subtype" => "a PDF/A output intent whose /S is not GTS_PDFA1",
    "output-intent-profile-missing" => "an output intent without /DestOutputProfile",
    "xmp-missing" => "PDF/A without XMP metadata in the catalog",
    "xmp-stream-invalid" => "/Metadata is not a stream of /Subtype /XML",
    "xmp-not-well-formed" => "the XMP metadata is not well-formed XML",
    "pdfa-id-missing" => "no pdfaid:part in the XMP metadata",
    "pdfa-id-mismatch" => "pdfaid:part or pdfaid:conformance does not fit the profile",
    "javascript" => "a JavaScript action",
    "launch-action" => "a Launch action",
    "named-action-not-allowed" => "a named action other than NextPage, PrevPage, FirstPage and LastPage",
    "remote-goto-action" => "a go-to action that opens another file",
    "uri-javascript" => "a URI action with the javascript: scheme",
    "embedded-file" => "an embedded file",
    "3d-content" => "a 3D annotation",
    "video-content" => "a movie annotation or action",
    "audio-content" => "a sound annotation or action",
    "multimedia-content" => "a screen, rich media or rendition object",
    "check-limit-reached" => "a bound on the work was reached; the report is incomplete",
    "content-not-checked" => "a part of the file was not checked",
    "writer-warning" => "a warning that printpdf raised while saving the document",
};

/// Checks the bytes of a PDF file. See the module documentation.
///
/// Does not panic. A file that cannot be read gives one `file-unreadable` finding.
pub fn preflight_bytes(bytes: &[u8], profile: PreflightProfile) -> PreflightReport {
    let mut sink = Sink::new(profile);
    file::check_header_and_tail(&mut sink, bytes);

    match Document::load_mem(bytes) {
        Err(e) => {
            if matches!(e, lopdf::Error::InvalidPassword) {
                let sev = sink
                    .forbidden(Some(Severity::Info))
                    .unwrap_or(Severity::Info);
                sink.push(
                    sev,
                    "encrypted",
                    Loc::default(),
                    "the file is encrypted and needs a password".to_string(),
                );
            }
            sink.error(
                "file-unreadable",
                Loc::default(),
                format!("the file could not be loaded: {e}"),
            );
        }
        Ok(doc) => {
            let mut run = Run::new(&doc, sink);
            structure::check_document(&mut run, bytes);
            sink = run.sink;
        }
    }
    sink.into_report()
}

impl PdfDocument {
    /// Checks the file that this document would become when saved with the default
    /// [`PdfSaveOptions`]. See [`preflight_bytes`] and the [module documentation](crate::preflight).
    pub fn preflight(&self, profile: PreflightProfile) -> PreflightReport {
        self.preflight_with_options(profile, &PdfSaveOptions::default())
    }

    /// Like [`preflight`](Self::preflight), saving with `opts`.
    ///
    /// Warnings and errors that the save reports become findings of the rule `writer-warning`.
    pub fn preflight_with_options(
        &self,
        profile: PreflightProfile,
        opts: &PdfSaveOptions,
    ) -> PreflightReport {
        let mut warnings = Vec::new();
        let bytes = self.save(opts, &mut warnings);
        let mut report = preflight_bytes(&bytes, profile);

        for w in warnings {
            let severity = match w.severity {
                PdfParseErrorSeverity::Error => Severity::Error,
                PdfParseErrorSeverity::Warning => Severity::Warning,
                PdfParseErrorSeverity::Info => continue,
            };
            report.add_finding(Finding {
                severity,
                rule: "writer-warning",
                message: w.msg,
                page: None,
                object: None,
                standard_reference: None,
            });
        }
        report
    }
}

/// Where a finding is.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Loc {
    /// Page, counted from 1.
    pub page: Option<usize>,
    pub obj: Option<ObjectId>,
}

impl Loc {
    pub fn obj(id: ObjectId) -> Loc {
        Loc {
            page: None,
            obj: Some(id),
        }
    }

    pub fn at(page: Option<usize>, obj: Option<ObjectId>) -> Loc {
        Loc { page, obj }
    }
}

/// A finding that is reported once per kind; later ones are only counted.
struct Aggregate {
    finding: Finding,
    more: usize,
}

/// Collects findings. It exists before the document is loaded, so that the checks of the raw
/// bytes can report into it.
pub(crate) struct Sink {
    pub profile: PreflightProfile,
    findings: Vec<Finding>,
    suppressed: usize,
    aggregates: BTreeMap<(&'static str, String), Aggregate>,
    limits: BTreeSet<&'static str>,
    pages: usize,
    objects: usize,
}

impl Sink {
    fn new(profile: PreflightProfile) -> Sink {
        Sink {
            profile,
            findings: Vec::new(),
            suppressed: 0,
            aggregates: BTreeMap::new(),
            limits: BTreeSet::new(),
            pages: 0,
            objects: 0,
        }
    }

    /// The severity for something that a standard forbids: an error for PDF/A and PDF/X, and
    /// `general` for the General profile (`None` for no finding at all).
    pub fn forbidden(&self, general: Option<Severity>) -> Option<Severity> {
        if self.profile.is_standard() {
            Some(Severity::Error)
        } else {
            general
        }
    }

    pub fn push(&mut self, severity: Severity, rule: &'static str, loc: Loc, message: String) {
        if self.findings.len() >= MAX_FINDINGS {
            self.suppressed = self.suppressed.saturating_add(1);
            return;
        }
        self.findings.push(Finding {
            severity,
            rule,
            message,
            page: loc.page,
            object: loc.obj,
            standard_reference: standard_reference(self.profile, rule),
        });
    }

    pub fn error(&mut self, rule: &'static str, loc: Loc, message: String) {
        self.push(Severity::Error, rule, loc, message);
    }

    pub fn warn(&mut self, rule: &'static str, loc: Loc, message: String) {
        self.push(Severity::Warning, rule, loc, message);
    }

    pub fn info(&mut self, rule: &'static str, loc: Loc, message: String) {
        self.push(Severity::Info, rule, loc, message);
    }

    /// Reports a finding once for each `(rule, key)`. Later findings with the same pair add to
    /// a count that is appended to the message.
    pub fn once(
        &mut self,
        severity: Severity,
        rule: &'static str,
        key: String,
        loc: Loc,
        message: String,
    ) {
        let profile = self.profile;
        self.aggregates
            .entry((rule, key))
            .and_modify(|a| a.more = a.more.saturating_add(1))
            .or_insert_with(|| Aggregate {
                finding: Finding {
                    severity,
                    rule,
                    message,
                    page: loc.page,
                    object: loc.obj,
                    standard_reference: standard_reference(profile, rule),
                },
                more: 0,
            });
    }

    pub fn limit(&mut self, what: &'static str) {
        self.limits.insert(what);
    }

    fn into_report(mut self) -> PreflightReport {
        for (_, mut a) in std::mem::take(&mut self.aggregates) {
            if a.more > 0 {
                a.finding.message = format!(
                    "{} ({} more of the same kind elsewhere)",
                    a.finding.message, a.more
                );
            }
            if self.findings.len() >= MAX_FINDINGS {
                self.suppressed = self.suppressed.saturating_add(1);
            } else {
                self.findings.push(a.finding);
            }
        }
        for what in &self.limits {
            // A limit finding is not dropped, so the list may exceed the cap by a few entries.
            self.findings.push(Finding {
                severity: Severity::Info,
                rule: "check-limit-reached",
                message: format!("{what}; the report may be incomplete"),
                page: None,
                object: None,
                standard_reference: None,
            });
        }

        let mut summary = PreflightSummary {
            suppressed: self.suppressed,
            pages_checked: self.pages,
            objects_visited: self.objects,
            limits_reached: self.limits.iter().copied().collect(),
            ..PreflightSummary::default()
        };
        for f in &self.findings {
            match f.severity {
                Severity::Error => summary.errors = summary.errors.saturating_add(1),
                Severity::Warning => summary.warnings = summary.warnings.saturating_add(1),
                Severity::Info => summary.infos = summary.infos.saturating_add(1),
            }
        }
        PreflightReport {
            profile: self.profile,
            findings: self.findings,
            summary,
            not_checked: NOT_CHECKED.to_vec(),
        }
    }
}

/// The clause of ISO 19005-1 for a rule. Only PDF/A-1b has references, and only for clauses
/// whose number is certain.
fn standard_reference(profile: PreflightProfile, rule: &str) -> Option<&'static str> {
    if profile != PreflightProfile::PdfA1b {
        return None;
    }
    let clause = match rule {
        "header-not-at-start" | "header-binary-comment-missing" => "ISO 19005-1 6.1.2",
        "encrypted" | "trailer-id-missing" | "eof-marker-missing" | "data-after-eof" => {
            "ISO 19005-1 6.1.3"
        }
        "stream-length-missing" | "stream-length-mismatch" | "stream-external-file" => {
            "ISO 19005-1 6.1.7"
        }
        "lzw-filter" => "ISO 19005-1 6.1.10",
        "embedded-file" => "ISO 19005-1 6.1.11",
        "optional-content" => "ISO 19005-1 6.1.13",
        "output-intent-subtype" | "output-intent-profile-missing" => "ISO 19005-1 6.2.2",
        "device-colour-without-output-intent" => "ISO 19005-1 6.2.3",
        "font-not-embedded" => "ISO 19005-1 6.3.4",
        "transparency-smask"
        | "transparency-alpha"
        | "transparency-blend-mode"
        | "transparency-group" => "ISO 19005-1 6.4",
        "javascript"
        | "launch-action"
        | "named-action-not-allowed"
        | "video-content"
        | "audio-content" => "ISO 19005-1 6.6.1",
        "xmp-missing" => "ISO 19005-1 6.7.2",
        "pdfa-id-missing" | "pdfa-id-mismatch" => "ISO 19005-1 6.7.11",
        _ => return None,
    };
    Some(clause)
}

/// What the checks of a loaded document share: the document, the findings and the bounds.
pub(crate) struct Run<'a> {
    pub doc: &'a Document,
    pub sink: Sink,
    pub content_left: usize,
    pub validation_left: usize,
    pub operators_left: usize,
    pub objects_left: usize,
    /// Whether the catalog has an output intent.
    pub has_output_intent: bool,
    /// The /N of the destination profile of each output intent.
    pub intent_components: Vec<i64>,
    /// Whether `DefaultGray`, `DefaultRGB` or `DefaultCMYK` is in a resource dictionary that
    /// was seen: index 0, 1, 2.
    pub default_spaces: [bool; 3],
    /// Visited objects, so that shared ones are checked once.
    pub seen: HashSet<(Seen, ObjectId)>,
    /// Dictionaries that are not indirect objects and were visited, by address.
    pub seen_dicts: HashSet<usize>,
    /// Streams that were reported as not decoding, so that a content check does not say it twice.
    pub decode_failed: HashSet<ObjectId>,
    /// Colour spaces that are indirect objects, read once.
    pub cs_cache: HashMap<ObjectId, colour::Space>,
    /// For `DeviceGray`, `DeviceRGB` and `DeviceCMYK`: `Some(page)` once it is used, with the
    /// first page that uses it.
    pub device_use: [Option<Option<usize>>; 3],
}

/// What was visited, for [`Run::seen`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Seen {
    Font,
    Form,
    Image,
    Resources,
    Profile,
}

impl<'a> Run<'a> {
    fn new(doc: &'a Document, sink: Sink) -> Run<'a> {
        Run {
            doc,
            sink,
            content_left: MAX_CONTENT_BYTES,
            validation_left: MAX_VALIDATION_BYTES,
            operators_left: MAX_OPERATORS,
            objects_left: MAX_OBJECTS,
            has_output_intent: false,
            intent_components: Vec::new(),
            default_spaces: [false; 3],
            seen: HashSet::new(),
            seen_dicts: HashSet::new(),
            decode_failed: HashSet::new(),
            cs_cache: HashMap::new(),
            device_use: [None; 3],
        }
    }

    pub fn profile(&self) -> PreflightProfile {
        self.sink.profile
    }

    /// Counts one visited object. Returns `false` when the bound is reached.
    pub fn visit(&mut self) -> bool {
        if self.objects_left == 0 {
            self.sink
                .limit("the limit of 2,000,000 visited objects was reached");
            return false;
        }
        self.objects_left -= 1;
        self.sink.objects = self.sink.objects.saturating_add(1);
        true
    }

    /// Whether `(what, id)` is new; it is remembered.
    pub fn first_visit(&mut self, what: Seen, id: ObjectId) -> bool {
        self.seen.insert((what, id))
    }
}

/// The dictionary of an object that is a dictionary or a stream.
pub(crate) fn dict_of(o: &Object) -> Option<&Dictionary> {
    match o {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}
