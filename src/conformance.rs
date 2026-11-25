//! PDF standards such as PDF/A and PDF/X, and what they require.
//!
//! Besides the general PDF format (ISO 32000), there are subset standards for specific
//! jobs: PDF/A for long-term archiving (ISO 19005), PDF/X for print production
//! (ISO 15930), PDF/UA for accessibility (ISO 14289), PDF/E for engineering (ISO 24517) and
//! PDF/VT for variable data printing (ISO 16612). A conforming file must follow extra
//! rules, for example embed all fonts, carry XMP metadata (an XML description of the
//! document stored in the file) or name the color profile it was prepared for (an output intent).
//!
//! [`PdfConformance`] is stored in [`PdfDocumentInfo::conformance`](crate::PdfDocumentInfo)
//! and chooses which of these are added when the document is saved.
//!
//! # What saving writes
//!
//! * PDF/X variants: the identifier of the standard (for example `"PDF/X-3:2002"`) in the
//!   `/GTS_PDFXVersion` entry of the document info dictionary and in the XMP metadata, and an
//!   output intent (`/S /GTS_PDFX`) with the embedded CMYK profile "Coated FOGRA39". PDF/X-1a:2001
//!   gets the output intent too, because the standard requires one.
//! * PDF/A variants ([`is_pdfa`](PdfConformance::is_pdfa)): an XMP packet with the identification
//!   (`pdfaid:part` and `pdfaid:conformance`) and the title, author, subject, keywords, creator,
//!   producer and dates of the info dictionary, written as the same values; an output intent
//!   (`/S /GTS_PDFA1`) with an embedded colour profile; the header version 1.4 (PDF/A-1) or 1.7
//!   (PDF/A-2 and 3); the trailer `/ID`; the binary comment after the header. No
//!   `/GTS_PDFXVersion`.
//! * The 14 built-in fonts are embedded for every variant that requires embedded fonts (all
//!   PDF/A and PDF/X variants, see [`must_embed_fonts`](PdfConformance::must_embed_fonts)), as
//!   TrueType subsets with the same names, the same encoding (`WinAnsiEncoding`) and the same
//!   text in the content stream, so text extraction does not change. `Symbol` and `ZapfDingbats`
//!   are the exception: the font files that printpdf bundles for them do not have the standard
//!   glyph set, so they stay unembedded and the save warns.
//!
//! # What is not guaranteed
//!
//! printpdf does not rewrite your content. When the document uses something that the chosen
//! standard does not allow, the save adds a warning ([`PdfWarnMsg`](crate::PdfWarnMsg)) and
//! writes the content as it is:
//!
//! * transparency (`/CA` or `/ca` below 1, soft masks, blend modes, transparency groups) in
//!   PDF/A-1 and PDF/X-1a and X-3 (it is allowed in PDF/A-2 and later);
//! * layers (optional content) in PDF/A-1, PDF/X-1a and PDF/X-3;
//! * the LZW filter, and the JPEG 2000 and JBIG2 filters in PDF/A-1;
//! * JavaScript, launch actions and embedded files (printpdf's own writer produces none of
//!   them, so this applies to content that came from a parsed file);
//! * device colour that the output intent does not cover: `DeviceRGB` needs an RGB profile and
//!   `DeviceCMYK` a CMYK one. printpdf embeds the sRGB profile, or the FOGRA39 profile when the
//!   document uses CMYK and no RGB. A document that uses both gets the sRGB profile and a
//!   warning. `DeviceRGB` in PDF/X-1a is reported the same way;
//! * symbols that the bundled font of a built-in font has no glyph for.
//!
//! Printpdf does not write tagged document structure (`/StructTreeRoot`, `/MarkInfo`), which
//! level a of PDF/A (1a, 2a) and PDF/UA need. For the `a` variants it writes
//! `pdfaid:conformance` as `B`, so the file claims level b, and warns on every save. For
//! [`A2U_2011_PDF_1_7`](PdfConformance::A2U_2011_PDF_1_7) it writes `B` as well, because printpdf
//! does not check that all text maps to Unicode.
//!
//! Printpdf never encrypts a file, so the PDF/A ban on encryption holds by construction. Fonts
//! that you embed yourself are written as they are (subset or complete).
//!
//! To see what a file is still missing, run the [preflight](crate::preflight)
//! (`PdfDocument::preflight`, `preflight_bytes` or `ppdf check`). It checks a subset of the rules
//! of PDF/A-1b, PDF/A-2b, PDF/X-1a:2001 and PDF/X-3:2002 and is not a certification. A file that
//! passes it has not been checked by veraPDF or by a printer's preflight.
//!
//! The default is [`PdfConformance::Custom`] with the default [`CustomPdfConformance`]: no
//! identifier, no XMP metadata and no output intent, which gives the smallest files.

use serde_derive::{Deserialize, Serialize};

/// A PDF standard that a document claims to follow.
///
/// Variant names read `<standard>_<year>_PDF_<version>`, with the PDF version of that
/// edition of the standard.
///
/// The enum is `#[serde(untagged)]`, so in JSON every variant except `Custom` is written
/// as `null`.
///
/// # Examples
///
/// ```
/// use printpdf::PdfConformance;
///
/// let c = PdfConformance::from_identifier_string("PDF/X-3:2002");
/// assert_eq!(c, PdfConformance::X3_2002_PDF_1_3);
/// assert!(c.must_have_xmp_metadata() && c.must_have_icc_profile());
/// assert_eq!(serde_json::to_string(&c).unwrap(), "null");
/// ```
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum PdfConformance {
    /// `PDF/A-1b`: basic archival conformance, many features restricted (ISO 19005-1).
    A1B_2005_PDF_1_4,
    /// `PDF/A-1a`: like 1b, plus document structure and Unicode text (ISO 19005-1).
    A1A_2005_PDF_1_4,
    /// `PDF/A-2`: adds JPEG 2000, transparency, layers and OpenType fonts (ISO 19005-2).
    A2_2011_PDF_1_7,
    /// `PDF/A-2a`: PDF/A-2 with document structure and Unicode text.
    A2A_2011_PDF_1_7,
    /// `PDF/A-2b`: PDF/A-2 with basic conformance.
    A2B_2011_PDF_1_7,
    /// `PDF/A-2u`: PDF/A-2 where all text maps to Unicode.
    A2U_2011_PDF_1_7,
    /// `PDF/A-3`: PDF/A-2 that may embed arbitrary files such as XML or CAD data (ISO 19005-3).
    A3_2012_PDF_1_7,
    /// `PDF/UA-1`: accessibility for screen readers and other assistive software (ISO 14289-1).
    UA_2014_PDF_1_6,
    /// `PDF/X-1a:2001`: print exchange with CMYK and spot colors, no ICC profiles (ISO 15930-1).
    X1A_2001_PDF_1_3,
    /// `PDF/X-3:2002`: like X-1a, and also allows calibrated color and ICC profiles
    /// (ISO 15930-3).
    X3_2002_PDF_1_3,
    /// `PDF/X-1a:2003`: revision of `PDF/X-1a:2001` based on PDF 1.4 (ISO 15930-4).
    X1A_2003_PDF_1_4,
    /// `PDF/X-3:2003`: revision of `PDF/X-3:2002` based on PDF 1.4 (ISO 15930-6).
    X3_2003_PDF_1_4,
    /// `PDF/X-4:2010`: colour-managed print exchange with transparency and layers
    /// (ISO 15930-7).
    X4_2010_PDF_1_4,
    /// `PDF/X-4p:2010`: X-4 whose ICC profile may be an external file, based on PDF 1.6.
    X4P_2010_PDF_1_6,
    /// `PDF/X-5g:2010`: X-4 that may reference graphics outside the file (ISO 15930-8).
    X5G_2010_PDF_1_6,
    /// `PDF/X-5pg`: X-5g combined with an external ICC profile for the output intent.
    X5PG_2010_PDF_1_6,
    /// `PDF/X-5n`: X-4p where the external output intent profile may use any color space.
    X5N_2010_PDF_1_6,
    /// `PDF/E-1:2008`: engineering documents with 3D objects and geospatial data
    /// (ISO 24517-1).
    E1_2008_PDF_1_6,
    /// `PDF/VT-1:2010`: variable and transactional printing, such as personalized mailings
    /// (ISO 16612-2).
    VT_2010_PDF_1_4,
    /// No standard in particular; the settings come from the [`CustomPdfConformance`].
    Custom(CustomPdfConformance),
}

/// The default is [`PdfConformance::Custom`] with the default [`CustomPdfConformance`],
/// which keeps the file small.
impl Default for PdfConformance {
    fn default() -> Self {
        Self::Custom(CustomPdfConformance::default())
    }
}

/// Settings of a [`PdfConformance::Custom`] profile, for documents that do not need to follow a
/// PDF standard, for example when the file should be as small as possible.
///
/// Serialized to JSON with camelCase field names; missing fields take the values of
/// [`CustomPdfConformance::default`], except where noted. The `PdfConformance` methods read
/// every field except `allows_default_fonts`, which is stored but not used.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPdfConformance {
    /// Text written as the standard's identifier. Default: empty.
    #[serde(default)]
    pub identifier: String,
    /// Whether 3D content is allowed. Default: `false`.
    #[serde(default)]
    pub allows_3d_content: bool,
    /// Whether video content is allowed. Default: `false`.
    #[serde(default)]
    pub allows_video_content: bool,
    /// Whether audio content is allowed. Default: `false`.
    #[serde(default)]
    pub allows_audio_content: bool,
    /// Whether embedded JavaScript is allowed. Default: `false`.
    #[serde(default)]
    pub allows_embedded_javascript: bool,
    /// Whether JPEG-compressed images are allowed. `true` from [`Default`], but `false` when
    /// the field is missing in deserialized data.
    #[serde(default = "default_false")]
    pub allows_jpeg_content: bool,
    /// Whether XMP metadata must be written. `false` from [`Default`] and when missing.
    #[serde(default = "default_false")]
    pub requires_xmp_metadata: bool,
    /// Whether the 14 built-in PDF fonts (Helvetica, ...) are allowed. They are not embedded,
    /// so printed output depends on the viewer's substitute. Default: `false`.
    #[serde(default)]
    pub allows_default_fonts: bool,
    /// Whether an output intent with an ICC color profile must be written. `false` from
    /// [`Default`] and when missing.
    #[serde(default = "default_false")]
    pub requires_icc_profile: bool,
    /// Whether layers (optional content) are allowed. `true` from [`Default`], but `false` when
    /// the field is missing in deserialized data.
    #[serde(default = "default_false")]
    pub allows_pdf_layers: bool,
}

fn default_false() -> bool {
    false
}

impl Default for CustomPdfConformance {
    fn default() -> Self {
        CustomPdfConformance {
            identifier: "".into(),
            allows_3d_content: false,
            allows_video_content: false,
            allows_audio_content: false,
            allows_embedded_javascript: false,
            allows_jpeg_content: true,
            requires_xmp_metadata: false,
            allows_default_fonts: false,
            requires_icc_profile: false,
            allows_pdf_layers: true,
        }
    }
}

impl PdfConformance {
    /// Parses an identifier such as `"PDF/A-2b:2011"` (see
    /// [`get_identifier_string`](Self::get_identifier_string)).
    ///
    /// An unknown string gives a [`PdfConformance::Custom`] with that identifier. This is how
    /// the `/GTS_PDFXVersion` entry of a parsed PDF is read.
    pub fn from_identifier_string(i: &str) -> Self {
        // the identifiers might not match the official names in all cases
        match i {
            "PDF/A-1b:2005" => PdfConformance::A1B_2005_PDF_1_4,
            "PDF/A-1a:2005" => PdfConformance::A1A_2005_PDF_1_4,
            "PDF/A-2:2011" => PdfConformance::A2_2011_PDF_1_7,
            "PDF/A-2a:2011" => PdfConformance::A2A_2011_PDF_1_7,
            "PDF/A-2b:2011" => PdfConformance::A2B_2011_PDF_1_7,
            "PDF/A-2u:2011" => PdfConformance::A2U_2011_PDF_1_7,
            "PDF/A-3:2012" => PdfConformance::A3_2012_PDF_1_7,
            "PDF/UA" => PdfConformance::UA_2014_PDF_1_6,
            "PDF/X-1a:2001" => PdfConformance::X1A_2001_PDF_1_3,
            "PDF/X-3:2002" => PdfConformance::X3_2002_PDF_1_3,
            "PDF/X-1a:2003" => PdfConformance::X1A_2003_PDF_1_4,
            "PDF/X-3:2003" => PdfConformance::X3_2003_PDF_1_4,
            "PDF/X-4" => PdfConformance::X4_2010_PDF_1_4,
            "PDF/X-4P" => PdfConformance::X4P_2010_PDF_1_6,
            "PDF/X-5G" => PdfConformance::X5G_2010_PDF_1_6,
            "PDF/X-5PG" => PdfConformance::X5PG_2010_PDF_1_6,
            "PDF/X-5N" => PdfConformance::X5N_2010_PDF_1_6,
            "PDF/E-1" => PdfConformance::E1_2008_PDF_1_6,
            "PDF/VT" => PdfConformance::VT_2010_PDF_1_4,
            i => PdfConformance::Custom(CustomPdfConformance {
                identifier: i.to_string(),
                ..Default::default()
            }),
        }
    }

    /// Identifier of the standard, such as `"PDF/X-3:2002"`. For `Custom` it is the
    /// `identifier` field.
    ///
    /// This is the string written to `/GTS_PDFXVersion` and to the XMP metadata of the PDF/X and
    /// the other non-PDF/A variants. The PDF/A variants write `pdfaid:part` and
    /// `pdfaid:conformance` instead.
    pub fn get_identifier_string(&self) -> String {
        let identifier = match *self {
            PdfConformance::A1B_2005_PDF_1_4 => "PDF/A-1b:2005",
            PdfConformance::A1A_2005_PDF_1_4 => "PDF/A-1a:2005",
            PdfConformance::A2_2011_PDF_1_7 => "PDF/A-2:2011",
            PdfConformance::A2A_2011_PDF_1_7 => "PDF/A-2a:2011",
            PdfConformance::A2B_2011_PDF_1_7 => "PDF/A-2b:2011",
            PdfConformance::A2U_2011_PDF_1_7 => "PDF/A-2u:2011",
            PdfConformance::A3_2012_PDF_1_7 => "PDF/A-3:2012",
            PdfConformance::UA_2014_PDF_1_6 => "PDF/UA",
            PdfConformance::X1A_2001_PDF_1_3 => "PDF/X-1a:2001",
            PdfConformance::X3_2002_PDF_1_3 => "PDF/X-3:2002",
            PdfConformance::X1A_2003_PDF_1_4 => "PDF/X-1a:2003",
            PdfConformance::X3_2003_PDF_1_4 => "PDF/X-3:2003",
            PdfConformance::X4_2010_PDF_1_4 => "PDF/X-4",
            PdfConformance::X4P_2010_PDF_1_6 => "PDF/X-4P",
            PdfConformance::X5G_2010_PDF_1_6 => "PDF/X-5G",
            PdfConformance::X5PG_2010_PDF_1_6 => "PDF/X-5PG",
            PdfConformance::X5N_2010_PDF_1_6 => "PDF/X-5N",
            PdfConformance::E1_2008_PDF_1_6 => "PDF/E-1",
            PdfConformance::VT_2010_PDF_1_4 => "PDF/VT",
            PdfConformance::Custom(ref c) => &c.identifier,
        };

        identifier.to_string()
    }

    /// Whether the standard permits 3D content: only PDF/E-1, or a `Custom` profile that
    /// allows it. The crate does not check document content against this.
    pub fn is_3d_content_allowed(&self) -> bool {
        match *self {
            PdfConformance::E1_2008_PDF_1_6 => true,
            PdfConformance::Custom(ref c) => c.allows_3d_content,
            _ => false,
        }
    }

    /// Whether video content is permitted: only for a `Custom` profile that allows it. The
    /// crate does not check document content against this.
    pub fn is_video_content_allowed(&self) -> bool {
        match *self {
            PdfConformance::Custom(ref c) => c.allows_video_content,
            _ => false,
        }
    }

    /// Whether audio content is permitted: only for a `Custom` profile that allows it. The
    /// crate does not check document content against this.
    pub fn is_audio_content_allowed(&self) -> bool {
        match *self {
            PdfConformance::Custom(ref c) => c.allows_audio_content,
            _ => false,
        }
    }

    /// Whether embedded JavaScript is permitted: only for a `Custom` profile that allows it.
    /// The crate does not check document content against this.
    pub fn is_javascript_content_allowed(&self) -> bool {
        match *self {
            PdfConformance::Custom(ref c) => c.allows_embedded_javascript,
            _ => false,
        }
    }

    /// Whether JPEG images are permitted: always, except for a `Custom` profile that
    /// disallows them. The crate does not check document content against this.
    pub fn is_jpeg_content_allowed(&self) -> bool {
        match *self {
            PdfConformance::Custom(ref c) => c.allows_jpeg_content,
            _ => true,
        }
    }

    /// Whether saving writes an XMP metadata stream: true for all PDF/A variants and for PDF/X-1a,
    /// X-3, X-4, X-4p, X-5g and X-5pg, and for a `Custom` profile with `requires_xmp_metadata`.
    /// The PDF/UA, PDF/E, PDF/VT and PDF/X-5n variants return false.
    pub fn must_have_xmp_metadata(&self) -> bool {
        match *self {
            PdfConformance::X1A_2001_PDF_1_3 => true,
            PdfConformance::X3_2002_PDF_1_3 => true,
            PdfConformance::X1A_2003_PDF_1_4 => true,
            PdfConformance::X3_2003_PDF_1_4 => true,
            PdfConformance::X4_2010_PDF_1_4 => true,
            PdfConformance::X4P_2010_PDF_1_6 => true,
            PdfConformance::X5G_2010_PDF_1_6 => true,
            PdfConformance::X5PG_2010_PDF_1_6 => true,
            PdfConformance::Custom(ref c) => c.requires_xmp_metadata,
            _ => self.is_pdfa(),
        }
    }

    /// Whether saving writes an output intent with an embedded ICC profile: true for every
    /// standard, and for a `Custom` profile with `requires_icc_profile`.
    ///
    /// PDF/X variants get `/S /GTS_PDFX` with the CMYK profile "Coated FOGRA39". PDF/A variants
    /// get `/S /GTS_PDFA1` with an sRGB profile, or with the FOGRA39 profile when the document
    /// uses DeviceCMYK and no DeviceRGB. Before this version PDF/X-1a:2001 returned false and
    /// had no output intent, which the standard requires.
    pub fn must_have_icc_profile(&self) -> bool {
        match *self {
            PdfConformance::Custom(ref c) => c.requires_icc_profile,
            _ => true,
        }
    }

    /// Whether this is one of the PDF/A variants (`A1B`, `A1A`, `A2`, `A2A`, `A2B`, `A2U`, `A3`).
    pub fn is_pdfa(&self) -> bool {
        self.pdfa_part().is_some()
    }

    /// The part of ISO 19005 that the variant names: 1, 2 or 3. `None` for everything that is not
    /// PDF/A. This is the number written as `pdfaid:part`.
    pub fn pdfa_part(&self) -> Option<u8> {
        match *self {
            PdfConformance::A1B_2005_PDF_1_4 | PdfConformance::A1A_2005_PDF_1_4 => Some(1),
            PdfConformance::A2_2011_PDF_1_7
            | PdfConformance::A2A_2011_PDF_1_7
            | PdfConformance::A2B_2011_PDF_1_7
            | PdfConformance::A2U_2011_PDF_1_7 => Some(2),
            PdfConformance::A3_2012_PDF_1_7 => Some(3),
            _ => None,
        }
    }

    /// The conformance level that the name of the variant asks for: `'A'`, `'B'` or `'U'`. `None`
    /// for everything that is not PDF/A. `PDF/A-2` and `PDF/A-3` without a level are `'B'`.
    ///
    /// What the file claims can be lower: printpdf writes no tagged structure, so it writes
    /// `pdfaid:conformance` as `B` for `'A'` and `'U'` (see the module docs).
    pub fn pdfa_level(&self) -> Option<char> {
        match *self {
            PdfConformance::A1A_2005_PDF_1_4 | PdfConformance::A2A_2011_PDF_1_7 => Some('A'),
            PdfConformance::A2U_2011_PDF_1_7 => Some('U'),
            PdfConformance::A1B_2005_PDF_1_4
            | PdfConformance::A2_2011_PDF_1_7
            | PdfConformance::A2B_2011_PDF_1_7
            | PdfConformance::A3_2012_PDF_1_7 => Some('B'),
            _ => None,
        }
    }

    /// Whether saving embeds the 14 built-in fonts: true for the PDF/A and PDF/X variants (their
    /// standards require every font to be embedded), false for the other variants and for
    /// `Custom`.
    pub fn must_embed_fonts(&self) -> bool {
        self.is_pdfa()
            || matches!(
                *self,
                PdfConformance::X1A_2001_PDF_1_3
                    | PdfConformance::X3_2002_PDF_1_3
                    | PdfConformance::X1A_2003_PDF_1_4
                    | PdfConformance::X3_2003_PDF_1_4
                    | PdfConformance::X4_2010_PDF_1_4
                    | PdfConformance::X4P_2010_PDF_1_6
                    | PdfConformance::X5G_2010_PDF_1_6
                    | PdfConformance::X5PG_2010_PDF_1_6
                    | PdfConformance::X5N_2010_PDF_1_6
            )
    }

    /// Whether the standard permits transparency (`/CA` and `/ca` below 1, soft masks, blend
    /// modes, transparency groups): false for PDF/A-1, PDF/X-1a and PDF/X-3, true for the other
    /// standards and for `Custom`. Saving warns when the document uses transparency and this is
    /// false; it does not remove it.
    pub fn is_transparency_allowed(&self) -> bool {
        !matches!(
            *self,
            PdfConformance::A1B_2005_PDF_1_4
                | PdfConformance::A1A_2005_PDF_1_4
                | PdfConformance::X1A_2001_PDF_1_3
                | PdfConformance::X3_2002_PDF_1_3
                | PdfConformance::X1A_2003_PDF_1_4
                | PdfConformance::X3_2003_PDF_1_4
        )
    }

    /// The version in the file header: `1.4` for PDF/A-1, `1.7` for PDF/A-2 and PDF/A-3, `1.3`
    /// for everything else.
    pub(crate) fn header_version(&self) -> &'static str {
        match self.pdfa_part() {
            Some(1) => "1.4",
            Some(_) => "1.7",
            None => "1.3",
        }
    }

    /// Whether the standard permits layers (optional content groups): false for PDF/A-1, PDF/X-1a
    /// and PDF/X-3, true for the other standards, and the `allows_pdf_layers` field for `Custom`.
    /// Saving warns when the document has layers and this is false; it does not remove them.
    pub fn is_layering_allowed(&self) -> bool {
        match self {
            PdfConformance::A1B_2005_PDF_1_4 => false,
            PdfConformance::A1A_2005_PDF_1_4 => false,
            PdfConformance::X1A_2001_PDF_1_3 => false,
            PdfConformance::X3_2002_PDF_1_3 => false,
            PdfConformance::X1A_2003_PDF_1_4 => false,
            PdfConformance::X3_2003_PDF_1_4 => false,
            PdfConformance::Custom(c) => c.allows_pdf_layers,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PdfConformance as C;

    #[test]
    fn pdfa_part_and_level() {
        let a1b = C::A1B_2005_PDF_1_4;
        assert_eq!((a1b.pdfa_part(), a1b.pdfa_level()), (Some(1), Some('B')));
        let a1a = C::A1A_2005_PDF_1_4;
        assert_eq!((a1a.pdfa_part(), a1a.pdfa_level()), (Some(1), Some('A')));
        let a2u = C::A2U_2011_PDF_1_7;
        assert_eq!((a2u.pdfa_part(), a2u.pdfa_level()), (Some(2), Some('U')));
        let a3 = C::A3_2012_PDF_1_7;
        assert_eq!((a3.pdfa_part(), a3.pdfa_level()), (Some(3), Some('B')));
        assert_eq!(C::X3_2002_PDF_1_3.pdfa_part(), None);
        assert_eq!(C::default().pdfa_level(), None);
        assert!(C::A2_2011_PDF_1_7.is_pdfa() && !C::UA_2014_PDF_1_6.is_pdfa());
    }

    #[test]
    fn what_each_standard_writes() {
        // XMP, an output intent and embedded fonts, including PDF/X-1a:2001, which has an output
        // intent since this version.
        for c in [
            C::A1B_2005_PDF_1_4,
            C::A2B_2011_PDF_1_7,
            C::X1A_2001_PDF_1_3,
            C::X3_2002_PDF_1_3,
        ] {
            assert!(
                c.must_have_xmp_metadata() && c.must_have_icc_profile() && c.must_embed_fonts(),
                "{c:?}"
            );
        }
        assert!(!C::default().must_have_xmp_metadata());
        assert!(!C::default().must_have_icc_profile());
        assert!(!C::default().must_embed_fonts());
        assert!(!C::A1B_2005_PDF_1_4.is_transparency_allowed());
        assert!(C::A2B_2011_PDF_1_7.is_transparency_allowed());
        assert!(!C::X1A_2001_PDF_1_3.is_transparency_allowed());
        assert!(C::X4_2010_PDF_1_4.is_transparency_allowed());
        assert_eq!(C::A1B_2005_PDF_1_4.header_version(), "1.4");
        assert_eq!(C::A2B_2011_PDF_1_7.header_version(), "1.7");
        assert_eq!(C::X1A_2001_PDF_1_3.header_version(), "1.3");
        assert_eq!(C::default().header_version(), "1.3");
    }
}
