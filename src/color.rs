use serde_derive::{Deserialize, Serialize};

use crate::IccProfileId;

/// Color space (enum for marking the number of bits a color has)
#[derive(Debug, Copy, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorSpace {
    Rgb,
    Rgba,
    Palette,
    Cmyk,
    Greyscale,
    GreyscaleAlpha,
}

impl ColorSpace {
    pub fn as_string(&self) -> &'static str {
        use self::ColorSpace::*;
        match self {
            Rgb => "DeviceRGB",
            Cmyk => "DeviceCMYK",
            Greyscale => "DeviceGray",
            Palette => "Indexed",
            Rgba | GreyscaleAlpha => "DeviceN",
        }
    }
}

impl From<image::ColorType> for ColorSpace {
    fn from(color_type: image::ColorType) -> Self {
        use image::ColorType::*;
        match color_type {
            L8 | L16 => ColorSpace::Greyscale,
            La8 | La16 => ColorSpace::GreyscaleAlpha,
            Rgb8 | Rgb16 => ColorSpace::Rgb,
            Rgba8 | Rgba16 => ColorSpace::Rgba,
            _ => ColorSpace::Greyscale, // unreachable
        }
    }
}

/// How many bits does a color have?
#[derive(Debug, Copy, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorBits {
    Bit1,
    Bit8,
    Bit16,
}

impl ColorBits {
    pub fn as_integer(&self) -> i64 {
        match self {
            ColorBits::Bit1 => 1,
            ColorBits::Bit8 => 8,
            ColorBits::Bit16 => 16,
        }
    }
}

/// Wrapper for Rgb, Cmyk and other color types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum Color {
    Rgb(Rgb),
    Cmyk(Cmyk),
    Greyscale(Greyscale),
    SpotColor(SpotColor),
}

impl Color {
    /// Consumes the color and converts into into a vector of numbers
    pub fn into_vec(&self) -> Vec<f32> {
        match self {
            Color::Rgb(rgb) => {
                vec![rgb.r, rgb.g, rgb.b]
            }
            Color::Cmyk(cmyk) => {
                vec![cmyk.c, cmyk.m, cmyk.y, cmyk.k]
            }
            Color::Greyscale(gs) => {
                vec![gs.percent]
            }
            Color::SpotColor(spot) => {
                vec![spot.c, spot.m, spot.y, spot.k]
            }
        }
    }

    /// Returns if the color has an icc profile attached
    pub fn get_icc_profile(&self) -> Option<&Option<IccProfileId>> {
        match *self {
            Color::Rgb(ref rgb) => Some(&rgb.icc_profile),
            Color::Cmyk(ref cmyk) => Some(&cmyk.icc_profile),
            Color::Greyscale(ref gs) => Some(&gs.icc_profile),
            Color::SpotColor(_) => None,
        }
    }
}

/// RGB color
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icc_profile: Option<IccProfileId>,
}

impl Rgb {
    pub fn new(r: f32, g: f32, b: f32, icc_profile: Option<IccProfileId>) -> Self {
        Self {
            r,
            g,
            b,
            icc_profile,
        }
    }
}

/// CMYK color
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cmyk {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icc_profile: Option<IccProfileId>,
}

impl Cmyk {
    /// Creates a new CMYK color
    pub fn new(c: f32, m: f32, y: f32, k: f32, icc_profile: Option<IccProfileId>) -> Self {
        Self {
            c,
            m,
            y,
            k,
            icc_profile,
        }
    }
}

/// Greyscale color
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Greyscale {
    pub percent: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icc_profile: Option<IccProfileId>,
}

impl Greyscale {
    pub fn new(percent: f32, icc_profile: Option<IccProfileId>) -> Self {
        Self {
            percent,
            icc_profile,
        }
    }
}

/// Spot colors are like Cmyk, but without color space. They are essentially "named" colors
/// from specific vendors - currently they are the same as a CMYK color.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotColor {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}

impl SpotColor {
    pub fn new(c: f32, m: f32, y: f32, k: f32) -> Self {
        Self { c, m, y, k }
    }
}

/// Type of the icc profile
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum IccProfileType {
    Cmyk,
    Rgb,
    Greyscale,
}

/// Icc profile
#[derive(Debug, Clone, PartialEq)]
pub struct IccProfile {
    /// Binary Icc profile
    pub icc: Vec<u8>,
    /// CMYK or RGB or LAB icc profile?
    pub icc_type: IccProfileType,
    /// Does the ICC profile have an "Alternate" version or not?
    pub has_alternate: bool,
    /// Does the ICC profile have an "Range" dictionary
    /// Really not sure why this is needed, but this is needed on the documents Info dictionary
    pub has_range: bool,
}

impl IccProfile {
    /// Creates a new Icc Profile
    pub fn new(icc: Vec<u8>, icc_type: IccProfileType) -> Self {
        Self {
            icc,
            icc_type,
            has_alternate: true,
            has_range: false,
        }
    }

    /// Does the ICC profile have an alternate version (such as "DeviceCMYk")?
    #[inline]
    pub fn with_alternate_profile(mut self, has_alternate: bool) -> Self {
        self.has_alternate = has_alternate;
        self
    }

    /// Does the ICC profile have an "Range" dictionary?
    #[inline]
    pub fn with_range(mut self, has_range: bool) -> Self {
        self.has_range = has_range;
        self
    }
}
