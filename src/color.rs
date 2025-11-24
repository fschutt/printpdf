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

#[cfg(feature = "images")]
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

/// Wrapper for Rgb, Cmyk and other color types. Note: ALL color values are normalized from 0.0 to
/// 1.0 NOT 0 - 255!
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum Color {
    /// RGB color, normalized from 0.0 to 1.0
    Rgb(Rgb),
    /// CMYK color, normalized from 0.0 to 1.0
    Cmyk(Cmyk),
    /// Greyscale color, normalized from 0.0 to 1.0
    Greyscale(Greyscale),
    /// (unimplemented) Spot color, currently encoded as CMYK, normalized from 0.0 to 1.0
    SpotColor(SpotColor),
}

impl Color {
    /// Returns true if color is not in 0.0 - 1.0 range
    pub fn is_out_of_range(&self) -> bool {
        match self {
            Color::Rgb(rgb) => rgb.is_out_of_range(),
            Color::Cmyk(cmyk) => cmyk.is_out_of_range(),
            Color::Greyscale(greyscale) => greyscale.is_out_of_range(),
            Color::SpotColor(spot_color) => spot_color.is_out_of_range(),
        }
    }

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

    pub fn get_svg_id(&self) -> String {
        match self {
            Color::Rgb(rgb) => {
                let r = (rgb.r * 255.0).round() as u8;
                let g = (rgb.g * 255.0).round() as u8;
                let b = (rgb.b * 255.0).round() as u8;
                format!("rgb({}, {}, {})", r, g, b)
            }
            Color::Cmyk(cmyk) => {
                let r = (1.0 - cmyk.c) * (1.0 - cmyk.k);
                let g = (1.0 - cmyk.m) * (1.0 - cmyk.k);
                let b = (1.0 - cmyk.y) * (1.0 - cmyk.k);
                let r = (r * 255.0).round() as u8;
                let g = (g * 255.0).round() as u8;
                let b = (b * 255.0).round() as u8;
                format!("rgb({}, {}, {})", r, g, b)
            }
            Color::Greyscale(gs) => {
                let gray = (gs.percent * 255.0).round() as u8;
                format!("rgb({}, {}, {})", gray, gray, gray)
            }
            Color::SpotColor(spot) => {
                // SpotColor is treated the same as CMYK.
                let r = (1.0 - spot.c) * (1.0 - spot.k);
                let g = (1.0 - spot.m) * (1.0 - spot.k);
                let b = (1.0 - spot.y) * (1.0 - spot.k);
                let r = (r * 255.0).round() as u8;
                let g = (g * 255.0).round() as u8;
                let b = (b * 255.0).round() as u8;
                format!("rgb({}, {}, {})", r, g, b)
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
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub r: f32,
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub g: f32,
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub b: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icc_profile: Option<IccProfileId>,
}

impl Rgb {
    /// Creates a new RGB color, NOTE: RGB has to be 0.0 - 1.0, not 0 - 255!
    pub fn new(r: f32, g: f32, b: f32, icc_profile: Option<IccProfileId>) -> Self {
        Self {
            r,
            g,
            b,
            icc_profile,
        }
    }

    /// Checks whether the color will be out of range (0.0 - 1.0)
    /// and lead to errors in the PDF encoding
    pub fn is_out_of_range(&self) -> bool {
        self.r < 0.0 || self.r > 1.0 || self.g < 0.0 || self.g > 1.0 || self.b < 0.0 || self.b > 1.0
    }
}

/// CMYK color
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cmyk {
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub c: f32,
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub m: f32,
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub y: f32,
    /// Note: This has to be 0.0 - 1.0, not 0 - 255!
    pub k: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icc_profile: Option<IccProfileId>,
}

impl Cmyk {
    /// Creates a new CMYK color, NOTE: CMYK has to be 0.0 - 1.0, not 0 - 255!
    pub fn new(c: f32, m: f32, y: f32, k: f32, icc_profile: Option<IccProfileId>) -> Self {
        Self {
            c,
            m,
            y,
            k,
            icc_profile,
        }
    }

    /// Checks whether the color will be out of range (0.0 - 1.0)
    /// and lead to errors in the PDF encoding
    pub fn is_out_of_range(&self) -> bool {
        self.c < 0.0
            || self.c > 1.0
            || self.m < 0.0
            || self.m > 1.0
            || self.y < 0.0
            || self.y > 1.0
            || self.k < 0.0
            || self.k > 1.0
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
    /// Creates a new Greyscale color, NOTE: Greyscale has to be 0.0 - 1.0, not 0 - 255!
    pub fn new(percent: f32, icc_profile: Option<IccProfileId>) -> Self {
        Self {
            percent,
            icc_profile,
        }
    }

    /// Checks whether the color will be out of range (0.0 - 1.0)
    /// and lead to errors in the PDF encoding
    pub fn is_out_of_range(&self) -> bool {
        self.percent < 0.0 || self.percent > 1.0
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
    /// Creates a new SpotColor, NOTE: SpotColor has to be 0.0 - 1.0, not 0 - 255!
    pub fn new(c: f32, m: f32, y: f32, k: f32) -> Self {
        Self { c, m, y, k }
    }

    /// Checks whether the color will be out of range (0.0 - 1.0)
    /// and lead to errors in the PDF encoding
    pub fn is_out_of_range(&self) -> bool {
        self.c < 0.0
            || self.c > 1.0
            || self.m < 0.0
            || self.m > 1.0
            || self.y < 0.0
            || self.y > 1.0
            || self.k < 0.0
            || self.k > 1.0
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
