//! Color module (CMYK or RGB). Shared between 2D and 3D module.

use *;

#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Rgb(Rgb),
    Cmyk(Cmyk),
    Grayscale(Grayscale),
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
            Color::Grayscale(gs) => { vec![gs.percent]},
            Color::SpotColor(spot) => { vec![spot.c, spot.m, spot.y, spot.k ]},
        }
    }

    /// Returns if the color has an icc profile attached
    pub fn get_icc_profile(&self)
    -> Option<&Option<IccProfile>>
    {
        match *self {
            Color::Rgb(ref rgb) => Some(&rgb.icc_profile),
            Color::Cmyk(ref cmyk) => Some(&cmyk.icc_profile),
            Color::Grayscale(ref gs) => Some(&gs.icc_profile),
            Color::SpotColor(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub icc_profile: Option<IccProfile>,
}

impl Rgb {

    pub fn new(r: f64, g: f64, b: f64, icc_profile: Option<IccProfile>)
    -> Self
    {
        Self { r, g, b, icc_profile }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cmyk {
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
    pub icc_profile: Option<IccProfile>,
}

impl Cmyk {
    /// Creates a new CMYK color
    pub fn new(c: f64, m: f64, y: f64, k: f64, icc_profile: Option<IccProfile>)
    -> Self
    {
        Self { c, m, y, k, icc_profile }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grayscale {
    pub percent: f64,
    pub icc_profile: Option<IccProfile>,
}

impl Grayscale {
    pub fn new(percent: f64, icc_profile: Option<IccProfile>)
    -> Self
    {
        Self { percent, icc_profile }
    }
}

/// Spot colors are like Cmyk, but without color space
/// They are essentially "named" colors from specific vendors
/// currently they are the same as a CMYK color.
#[derive(Debug, Clone, PartialEq)]
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
