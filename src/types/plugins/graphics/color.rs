//! Color module (CMYK or RGB). Shared between 2D and 3D module.

use lopdf;
use glob_defines::*;
use indices::IccProfileIndex;
use traits::IntoPdfStreamOperation;

/// Tuple for differentiating outline and fill colors
#[derive(Debug, Clone, PartialEq)]
pub enum PdfColor {
    FillColor(Color),
    OutlineColor(Color),
}

impl IntoPdfStreamOperation for PdfColor {

    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        use lopdf::Object::*;
        use lopdf::content::Operation;

        // todo: incorporate ICC profile instead of just setting the default device cmyk color space
        let (color_identifier, color_vec) = {
            use self::PdfColor::*;
            match *self {
                FillColor(fill) => {
                    let ci = match fill {
                        Color::Rgb(_) => { OP_COLOR_SET_FILL_CS_DEVICERGB }
                        Color::Cmyk(_) => { OP_COLOR_SET_FILL_CS_DEVICECMYK }
                        Color::Greyscale(_) => { OP_COLOR_SET_FILL_CS_DEVICEGRAY }
                        Color::SpotColor(_) => { OP_COLOR_SET_FILL_CS_DEVICECMYK }
                    };
                    let cvec = fill.into_vec().into_iter().map(move |float| Real(float)).collect();
                    (ci, cvec)
                },
                OutlineColor(outline) => {
                    let ci = match outline {
                        Color::Rgb(_) => { OP_COLOR_SET_STROKE_CS_DEVICERGB }
                        Color::Cmyk(_) => { OP_COLOR_SET_STROKE_CS_DEVICECMYK }
                        Color::Greyscale(_) => { OP_COLOR_SET_STROKE_CS_DEVICEGRAY }
                        Color::SpotColor(_) => { OP_COLOR_SET_STROKE_CS_DEVICECMYK }
                    };

                    let cvec = outline.into_vec().into_iter().map(move |float| Real(float)).collect();
                    (ci, cvec)
                }
            }
        };

        vec![Operation::new(color_identifier, color_vec)]
    }
}

/// Color space (enum for marking the number of bits a color has)
#[derive(Debug)]
pub enum ColorSpace {
    Rgb,
    Cmyk,
    Greyscale,
}

impl Into<&'static str> for ColorSpace {
    fn into(self)
    -> &'static str
    {
        match self {
            ColorSpace::Rgb => "DeviceRGB",
            ColorSpace::Cmyk => "DeviceCMYK",
            ColorSpace::Greyscale => "DeviceGray",
        }
    }
}

/// How many bits does a color have?
#[derive(Debug)]
pub enum ColorBits {
    Bit1,
    Bit8,
    Bit16,
}

impl Into<i64> for ColorBits {
    fn into(self)
    -> i64
    {
        match self {
            ColorBits::Bit1 => 1,
            ColorBits::Bit8 => 8,
            ColorBits::Bit16 => 16,
        }
    }
}

/// Wrapper for Rgb, Cmyk and other color types
#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Rgb(Rgb),
    Cmyk(Cmyk),
    Greyscale(Greyscale),
    SpotColor(SpotColor)
}

impl Color {
    
    /// Consumes the color and converts into into a vector of numbers
    pub fn into_vec(self)
    -> Vec<f64>
    {
        match self {
            Color::Rgb(rgb) => { vec![rgb.r, rgb.g, rgb.b ]},
            Color::Cmyk(cmyk) => { vec![cmyk.c, cmyk.m, cmyk.y, cmyk.k ]},
            Color::Greyscale(gs) => { vec![gs.percent]},
            Color::SpotColor(spot) => { vec![spot.c, spot.m, spot.y, spot.k ]},
        }
    }

    /// Returns if the color has an icc profile attached
    pub fn get_icc_profile(&self)
    -> Option<&Option<IccProfileIndex>>
    {
        match *self {
            Color::Rgb(ref rgb) => Some(&rgb.icc_profile),
            Color::Cmyk(ref cmyk) => Some(&cmyk.icc_profile),
            Color::Greyscale(ref gs) => Some(&gs.icc_profile),
            Color::SpotColor(_) => None,
        }
    }
}

/// RGB color
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub icc_profile: Option<IccProfileIndex>,
}

impl Rgb {

    pub fn new(r: f64, g: f64, b: f64, icc_profile: Option<IccProfileIndex>)
    -> Self
    {
        Self { r, g, b, icc_profile }
    }
}


/// CMYK color
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Cmyk {
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
    pub icc_profile: Option<IccProfileIndex>,
}

impl Cmyk {
    /// Creates a new CMYK color
    pub fn new(c: f64, m: f64, y: f64, k: f64, icc_profile: Option<IccProfileIndex>)
    -> Self
    {
        Self { c, m, y, k, icc_profile }
    }
}


/// Greyscale color
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Greyscale {
    pub percent: f64,
    pub icc_profile: Option<IccProfileIndex>,
}

impl Greyscale {
    pub fn new(percent: f64, icc_profile: Option<IccProfileIndex>)
    -> Self
    {
        Self { percent, icc_profile }
    }
}


/// Spot color
/// Spot colors are like Cmyk, but without color space
/// They are essentially "named" colors from specific vendors
/// currently they are the same as a CMYK color.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SpotColor {
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
}

impl SpotColor {
    pub fn new(c: f64, m: f64, y: f64, k: f64)
    -> Self
    {
        Self { c, m, y, k }
    }
}
