//! Module regulating the comparison and feature sets / allowed plugins of a PDF document
//!
//! NOTE: All credit to Wikipedia:
//!
//! [PDF/X Versions](https://en.wikipedia.org/wiki/PDF/X)
//!
//! [PDF/A Versions](https://en.wikipedia.org/wiki/PDF/A)

use serde_derive::{Deserialize, Serialize};

/// List of (relevant) PDF versions
/// Please note the difference between **PDF/A** (archiving), **PDF/UA** (universal acessibility),
/// **PDF/X** (printing), **PDF/E** (engineering / CAD), **PDF/VT** (large volume transactions with
/// repeated content)
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum PdfConformance {
    /// `PDF/A-1b` basic PDF, many features restricted
    A1B_2005_PDF_1_4,
    /// `PDF/A-1a` language specification, hierarchical document structure,
    /// character mappings to unicode, descriptive text for images
    A1A_2005_PDF_1_4,
    /// `PDF/A-2:2011` - JPEG compression, transpareny, layering, OpenType fonts
    A2_2011_PDF_1_7,
    /// `PDF/A-2a:2011`
    A2A_2011_PDF_1_7,
    /// `PDF/A-2b:2011`
    A2B_2011_PDF_1_7,
    /// `PDF/A-2u:2011` - requires all text to be Unicode
    A2U_2011_PDF_1_7,
    /// `PDF/A-3` - like A2 but with embedded files (XML, CAD, etc.)
    A3_2012_PDF_1_7,
    /// `PDF/UA-1` extra functions for accessibility (blind, screenreaders, search, dynamic layout)
    UA_2014_PDF_1_6,
    /// `PDF/X-1a:2001` no ICC profiles
    X1A_2001_PDF_1_3,
    /// `PDF/X-3:2002` allows CMYK, spot, calibrated (managed) RGB, CIELAB, + ICC Profiles
    X3_2002_PDF_1_3,
    /// `PDF/X-1a:2003` Revision of `PDF/X-1a:2001` based on PDF 1.4
    X1A_2003_PDF_1_4,
    /// `PDF/X-3:2003` Revision of `PDF/X-3:2002` based on PDF 1.4
    X3_2003_PDF_1_4,
    /// `PDF/X-4:2010` Colour-managed, CMYK, gray, RGB or spot colour data are supported
    /// as well as PDF transparency and optional content (layers)
    X4_2010_PDF_1_4,
    /// `PDF/X-4p:2010` Same as the above X-4:2010, but may reference an ICC profile from
    /// an external file, and it's based on PDF 1.6
    X4P_2010_PDF_1_6,
    /// `PDF/X-5g:2010` An extension of PDF/X-4 that enables the use of external graphical
    /// content. This can be described as OPI-like (Open Prepress Interface) workflows.
    /// Specifically this allows graphics to be referenced that are outside the PDF
    X5G_2010_PDF_1_6,
    /// `PDF/X-5pg` An extension of PDF/X-4p that enables the use of external graphical
    /// content in conjunction with a reference to an external ICC Profile for the output intent.
    X5PG_2010_PDF_1_6,
    /// `PDF/X-5n` An extension of PDF/X-4p that allows the externally supplied ICC
    /// Profile for the output intent to use a color space other than Greyscale, RGB and CMYK.
    X5N_2010_PDF_1_6,
    /// `PDF/E-1:2008` 3D Objects, geospatial, etc.
    E1_2008_PDF_1_6,
    /// `PDF/VT-1:2010` Basically a way to make a incomplete PDF as a template and the RIP program
    /// is set up in a way that it can easily inject data into the PDF, for high-throughput PDFs
    /// (like postcards, stamps), that require customization before printing
    VT_2010_PDF_1_4,
    /// Custom PDF conformance, to allow / disallow options. This allows for making very small
    /// documents, for example
    Custom(CustomPdfConformance),
}

// default: save on file size
impl Default for PdfConformance {
    fn default() -> Self {
        Self::Custom(CustomPdfConformance::default())
    }
}

/// Allows building custom conformance profiles. This is useful if you want very small documents for
/// example and you don't __need__ conformance with any PDF standard, you just want a PDF file.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPdfConformance {
    /// Identifier for this conformance
    ///
    /// Default: __""__
    #[serde(default)]
    pub identifier: String,
    /// Does this standard allow 3d content?
    ///
    /// Default: __false__
    #[serde(default)]
    pub allows_3d_content: bool,
    /// Does this standard allow video content?
    ///
    /// Default: __false__
    #[serde(default)]
    pub allows_video_content: bool,
    /// Does this standard allow audio content
    ///
    /// Default: __false__
    #[serde(default)]
    pub allows_audio_content: bool,
    /// Does this standard allow enbedded JS?
    ///
    /// Default: __false__
    #[serde(default)]
    pub allows_embedded_javascript: bool,
    /// Does this standard allow enbedding JPEG files?
    ///
    /// Default: __true__
    #[serde(default = "f_true")]
    pub allows_jpeg_content: bool,
    /// Does this standard require XMP metadata to be set?
    ///
    /// Default: __true__
    #[serde(default = "f_true")]
    pub requires_xmp_metadata: bool,
    /// Does this standard allow the default PDF fonts (Helvetica, etc.)
    ///
    /// _(please don't enable this if you do any work that has to be printed accurately)_
    ///
    /// Default: __false__
    #[serde(default)]
    pub allows_default_fonts: bool,
    /// Does this standard require an ICC profile to be embedded for color management?
    ///
    /// Default: __true__
    #[serde(default = "f_true")]
    pub requires_icc_profile: bool,
    /// Does this standard allow PDF layers?
    ///
    /// Default: __true__
    #[serde(default = "f_true")]
    pub allows_pdf_layers: bool,
}

fn f_true() -> bool {
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
    /// Get the identifier string for PDF
    pub fn from_identifier_string(i: &str) -> Self {
        // todo: these identifiers might not be correct in all cases
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

    /// Get the identifier string for PDF
    pub fn get_identifier_string(&self) -> String {
        // todo: these identifiers might not be correct in all cases
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

    /// __STUB__: Detects if the PDF has 3D content, but the
    /// conformance to the given PDF standard does not allow it.
    pub fn is_3d_content_allowed(&self) -> bool {
        match *self {
            PdfConformance::E1_2008_PDF_1_6 => true,
            PdfConformance::Custom(ref c) => c.allows_3d_content,
            _ => false,
        }
    }

    /// Does this conformance level allow video
    pub fn is_video_content_allowed(&self) -> bool {
        // todo
        match *self {
            PdfConformance::Custom(ref c) => c.allows_video_content,
            _ => false,
        }
    }

    /// __STUB__: Detects if the PDF has audio content, but the
    /// conformance to the given PDF standard does not allow it.
    pub fn is_audio_content_allowed(&self) -> bool {
        // todo
        match *self {
            PdfConformance::Custom(ref c) => c.allows_audio_content,
            _ => false,
        }
    }

    /// __STUB__: Detects if the PDF has 3D content, but the
    /// conformance to the given PDF standard does not allow it.
    pub fn is_javascript_content_allowed(&self) -> bool {
        // todo
        match *self {
            PdfConformance::Custom(ref c) => c.allows_embedded_javascript,
            _ => false,
        }
    }

    /// __STUB__: Detects if the PDF has JPEG images, but the
    /// conformance to the given PDF standard does not allow it
    pub fn is_jpeg_content_allowed(&self) -> bool {
        // todo
        match *self {
            PdfConformance::Custom(ref c) => c.allows_jpeg_content,
            _ => true,
        }
    }

    /// Detects if the PDF must have XMP metadata
    /// if it has to conform to the given PDF Standard
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
            _ => false,
        }
    }

    /// Check if the conformance level must have an ICC Profile
    pub fn must_have_icc_profile(&self) -> bool {
        // todo
        match *self {
            PdfConformance::X1A_2001_PDF_1_3 => false,
            PdfConformance::Custom(ref c) => c.requires_icc_profile,
            _ => true,
        }
    }

    /// __STUB__: Detects if the PDF has layering (optional content groups),
    /// but the conformance to the given PDF standard does not allow it.
    pub fn is_layering_allowed(&self) -> bool {
        match self {
            PdfConformance::X1A_2001_PDF_1_3 => false,
            PdfConformance::X3_2002_PDF_1_3 => false,
            PdfConformance::X1A_2003_PDF_1_4 => false,
            PdfConformance::X3_2003_PDF_1_4 => false,
            PdfConformance::Custom(c) => c.allows_pdf_layers,
            _ => true,
        }
    }
}
