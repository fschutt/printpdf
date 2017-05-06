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
pub enum PdfConformance {

    // --- PDF/A

    /// PDF/A-1b - PDF 1.4 - **b**asic conformity level, many features restricted
    Pdf_A1B_ISO_19005_1_2005_Pdf_1_4,
    /// PDF/A-1a - PDF 1.4 - **a**ccessibility level, has: language specification,
    /// hierarchical document structure, character mappings to unicode, descriptive text for images
    Pdf_A1A_ISO_19005_1_2005_Pdf_1_4,
    /// PDF/A-2 - PDF 1.5 - 1.7
    Pdf_A2_ISO_19005_2_2011_Pdf_1_4,
    /// PD
    Pdf_A2_ISO_19005_2_2011_Pdf_1_5,
    /// 
    Pdf_A2_ISO_19005_2_2011_Pdf_1_6,
    /// PDF/A-3 - PDF 1.7 - like A2 but with embedded files (XML, CAD, etc.)
    Pdf_A3_ISO_32000_3_2012_Pdf_1_7,

    // --- PDF/UA

    /// PDF/UA-1 - PDF 1.6 - Accessibility functions for blind, search, dynamic layout, etc.
    Pdf_UA_ISO_14289_1_2014_Pdf_1_6,

    // --- PDF/X

    /// PDF/X-1a:2001 - PDF 1.3 - blind exchange in CMYK + spot colors
    Pdf_X1A_ISO_15930_1_2001_Pdf_1_3,
    /// PDF/X-3:2002 - PDF 1.3 - allows CMYK, spot, calibrated (managed) RGB, CIELAB, + ICC Profiles
    Pdf_X3_ISO_15930_3_2002_Pdf_1_3,
    /// PDF/X-1a:2003 -PDF 1.4 - Revision of PDF/X-1a:2001 based on PDF 1.4
    Pdf_X3_ISO_15930_4_2004_Pdf_1_4,
    /// PDF/X-3:2003 - PDF 1.4 - Revision of PDF/X-3:2002 based on PDF 1.4
    Pdf_X3_ISO_15930_6_2003_Pdf_1_4,
    /// PDF/X-4:2010 - PDF 1.4 - Colour-managed, CMYK, gray, RGB or spot colour data are supported
    /// as well as PDF transparency and optional content (layers)
    Pdf_X4_ISO_15930_7_2010_Pdf_1_4,
    /// PDF/X-4p:2010 - PDF 1.6 - Same as the above X-4:2010, but may reference an ICC profile from
    /// an external file, and it's based on PDF 1.6
    Pdf_X4P_ISO_15930_7_2010_Pdf_1_6,
    /// PDF/X-5g:2010 - PDF 1.6 - An extension of PDF/X-4 that enables the use of external graphical
    /// content. This can be described as OPI-like (Open Prepress Interface) workflows.
    /// Specifically this allows graphics to be referenced that are outside the PDF
    Pdf_X5G_ISO_15930_8_2010_Pdf_1_6,
    /// PDF/X-5pg - PDF 1.6 - An extension of PDF/X-4p that enables the use of external graphical 
    /// content in conjunction with a reference to an external ICC Profile for the output intent.
    Pdf_X5PG_ISO_15930_8_2010_Pdf_1_6,
    /// PDF/X-5n - PDF 1.6 - An extension of PDF/X-4p that allows the externally supplied ICC 
    /// Profile for the output intent to use a color space other than Grayscale, RGB and CMYK.
    Pdf_X5N_ISO_15930_8_2010_Pdf_1_6,

    // --- PDF/E

    /// PDF/E-1:2008 - PDF 1.6 - 
    Pdf_E1_ISO_24517_1_2008_Pdf_1_6,

    // --- PDF/VT

    /// PDF/VT-1:2010 - 
    Pdf_VT_
}