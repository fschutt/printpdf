//! Module regulating the comparison and feature sets / allowed plugins of a PDF document
//! 
//! NOTE: All credit to Wikipedia:
//! 
//! [PDF/X Versions](https://en.wikipedia.org/wiki/PDF/X)
//!
//! [PDF/A Versions](https://en.wikipedia.org/wiki/PDF/A)

/// List of (relevant) PDF versions
/// Please note the difference between **PDF/A** (archiving), **PDF/UA** (universal acessibility),
/// **PDF/X** (printing), **PDF/E** (engineering / CAD), **PDF/VT** (large volume transactions with 
/// repeated content)
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[allow(non_camel_case_types)]
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
}

impl PdfConformance {

    /// Get the identifier string for PDF
    pub fn get_identifier_string(&self)
    -> String
    {
        let identifier = match *self {
            PdfConformance::A1B_2005_PDF_1_4  => "PDF/A-1b:2005",
            PdfConformance::A1A_2005_PDF_1_4  => "PDF/A-1a:2005",
            PdfConformance::A2_2011_PDF_1_7   => "PDF/A-2:2011",
            PdfConformance::A2A_2011_PDF_1_7  => "PDF/A-2a:2011",
            PdfConformance::A2B_2011_PDF_1_7  => "PDF/A-2b:2011",
            PdfConformance::A2U_2011_PDF_1_7  => "PDF/A-2u:2011",
            PdfConformance::A3_2012_PDF_1_7   => "PDF/A-3:2012",
            PdfConformance::UA_2014_PDF_1_6   => "PDF/UA",
            PdfConformance::X1A_2001_PDF_1_3  => "PDF/X-1a:2001",
            PdfConformance::X3_2002_PDF_1_3   => "PDF/X-3:2002",
            PdfConformance::X1A_2003_PDF_1_4  => "PDF/X-1a:2003",
            PdfConformance::X3_2003_PDF_1_4   => "PDF/X-3:2003",
            PdfConformance::X4_2010_PDF_1_4   => "PDF/X-4",
            PdfConformance::X4P_2010_PDF_1_6  => "PDF/X-4",
            PdfConformance::X5G_2010_PDF_1_6  => "PDF/X-5",
            PdfConformance::X5PG_2010_PDF_1_6 => "PDF/X-5",
            PdfConformance::X5N_2010_PDF_1_6  => "PDF/X-5",
            PdfConformance::E1_2008_PDF_1_6   => "PDF/E-1",
            PdfConformance::VT_2010_PDF_1_4   => "PDF/VT",
        };

        identifier.to_string()
    }

    pub fn is_3d_content_allowed(&self)
    -> bool
    {
        match *self {
           PdfConformance::E1_2008_PDF_1_6   => true,
           _ => false,
        }
    }

    /// Does this conformance level allow video
    pub fn is_video_content_allowed(&self)
    -> bool
    {
        // todo
        false
    }

    /// Does this conformance level allow video
    pub fn is_audio_content_allowed(&self)
    -> bool
    {
        // todo
        false
    }

    pub fn is_javascript_content_allowed(&self)
    -> bool
    {
        // todo
        false
    }

    pub fn is_jpeg_content_allowed(&self)
    -> bool
    {
        // todo
        false
    }

    pub fn must_have_xmp_metadata(&self)
    -> bool
    {
        match *self {
            PdfConformance::X1A_2001_PDF_1_3  => { true },
            PdfConformance::X3_2002_PDF_1_3   => { true },
            PdfConformance::X1A_2003_PDF_1_4  => { true },
            PdfConformance::X3_2003_PDF_1_4   => { true },
            PdfConformance::X4_2010_PDF_1_4   => { true },
            PdfConformance::X4P_2010_PDF_1_6  => { true },
            PdfConformance::X5G_2010_PDF_1_6  => { true },
            PdfConformance::X5PG_2010_PDF_1_6 => { true },
            _                                 => { false },
        }
    }

    /// Check if the conformance level must have an ICC Profile
    pub fn must_have_icc_profile(&self)
    -> bool
    {
        // todo
        match *self {
            PdfConformance::X1A_2001_PDF_1_3  => { false },
            _                                 => { true },
        }
    }

    pub fn is_layering_allowed(&self)
    -> bool
    {
        match *self {
            PdfConformance::X1A_2001_PDF_1_3  => { false },
            PdfConformance::X3_2002_PDF_1_3   => { false },
            PdfConformance::X1A_2003_PDF_1_4  => { false },
            PdfConformance::X3_2003_PDF_1_4   => { false },
            _                                 => { true },
        }
    }
}