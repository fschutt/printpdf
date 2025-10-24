use core::fmt;
use std::io::Cursor;

use base64::Engine;
use image::{DynamicImage, GenericImageView};
use serde::de::Error;
use serde_derive::{Deserialize, Serialize};

use crate::{ColorBits, ColorSpace, PdfWarnMsg};

/// Options for optimizing images in PDF
#[derive(Debug, Clone, Serialize, PartialOrd, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOptimizationOptions {
    /// Quality level for lossy compression (0.0-1.0)
    #[serde(default = "default_quality", skip_serializing_if = "Option::is_none")]
    pub quality: Option<f32>,
    /// Maximum image size (e.g. "300kb")
    #[serde(
        default = "default_max_img_size",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_image_size: Option<String>,
    /// Whether to apply dithering to greyscale images
    #[serde(
        default = "default_dither_greyscale",
        skip_serializing_if = "Option::is_none"
    )]
    pub dither_greyscale: Option<bool>,
    /// Automatically convert the image to greyscale (only done if auto_optimize = true)
    #[serde(
        default = "default_convert_to_greyscale",
        skip_serializing_if = "Option::is_none"
    )]
    pub convert_to_greyscale: Option<bool>,
    /// Auto-optimize images (remove alpha if not needed, detect greyscale)
    #[serde(
        default = "default_auto_optimize",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_optimize: Option<bool>,
    /// Preferred compression format
    #[serde(default = "default_format", skip_serializing_if = "Option::is_none")]
    pub format: Option<ImageCompression>,
}

const fn default_quality() -> Option<f32> {
    Some(0.85)
}

fn default_max_img_size() -> Option<String> {
    Some("2MB".to_string())
}

const fn default_convert_to_greyscale() -> Option<bool> {
    Some(false)
}

const fn default_dither_greyscale() -> Option<bool> {
    None
}

const fn default_auto_optimize() -> Option<bool> {
    Some(true)
}

const fn default_format() -> Option<ImageCompression> {
    Some(ImageCompression::Auto)
}

impl Default for ImageOptimizationOptions {
    fn default() -> Self {
        ImageOptimizationOptions {
            quality: default_quality(),
            convert_to_greyscale: default_convert_to_greyscale(),
            max_image_size: default_max_img_size(),
            dither_greyscale: default_dither_greyscale(),
            auto_optimize: default_auto_optimize(),
            format: default_format(),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, PartialOrd, PartialEq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageCompression {
    /// Automatic selection based on image content
    #[default]
    Auto,
    /// JPEG compression (DCT filter)
    Jpeg,
    /// JPEG2000 compression (JPX filter - TODO: uses DCT filter)
    Jpeg2000,
    /// Flate compression (lossless)
    Flate,
    /// LZW compression (lossless)
    Lzw,
    /// Run Length encoding
    RunLength,
    /// None (raw)
    None,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct RawImage {
    pub pixels: RawImageData,
    pub width: usize,
    pub height: usize,
    pub data_format: RawImageFormat,
    pub tag: Vec<u8>,
}

impl serde::Serialize for RawImage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Cycle through all output image formats until one succeeds.
        let output_formats = [
            OutputImageFormat::Png,
            OutputImageFormat::Jpeg,
            OutputImageFormat::Gif,
            OutputImageFormat::Webp,
            OutputImageFormat::Pnm,
            OutputImageFormat::Tiff,
            OutputImageFormat::Tga,
            OutputImageFormat::Bmp,
            OutputImageFormat::Avif,
        ];
        let (bytes, fmt) = self
            .encode_to_bytes(&output_formats)
            .map_err(serde::ser::Error::custom)?;
        let base64_str = base64::prelude::BASE64_STANDARD.encode(&bytes);
        let data_url = format!("data:{};base64,{}", fmt.mime_type(), base64_str);
        serializer.serialize_str(&data_url)
    }
}

impl<'de> serde::Deserialize<'de> for RawImage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        // If the string is a data URL (e.g. "data:image/png;base64,..."),
        // strip the header and keep the base64 payload.
        let base64_part = if s.starts_with("data:") {
            s.find(',')
                .map(|idx| &s[idx + 1..])
                .ok_or_else(|| D::Error::custom("Invalid data URL: missing comma"))?
        } else {
            &s
        };
        let bytes = base64::prelude::BASE64_STANDARD
            .decode(base64_part)
            .map_err(serde::de::Error::custom)?;

        Self::decode_from_bytes(&bytes, &mut Vec::new()).map_err(serde::de::Error::custom)
    }
}

struct RawImageU8 {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub data_format: RawImageFormat,
}

impl fmt::Debug for RawImageU8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawImageU8")
            .field("pixels", &self.pixels.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .field("data_format", &self.data_format)
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
#[serde(rename_all = "lowercase")]
pub enum RawImageFormat {
    R8,
    RG8,
    RGB8,
    RGBA8,
    R16,
    RG16,
    RGB16,
    RGBA16,
    BGR8,
    BGRA8,
    RGBF32,
    RGBAF32,
}

impl RawImageFormat {
    pub fn reduce_to_rgb(&self) -> Self {
        use RawImageFormat::*;
        match self {
            RGBA8 => RGB8,
            RGBA16 => RGB16,
            RGBAF32 => RGBF32,
            RG8 => R8,
            RG16 => R16,
            other => *other,
        }
    }

    #[allow(unused)]
    #[cfg(feature = "html")]
    fn from_internal(f: &azul_core::app_resources::RawImageFormat) -> Self {
        use azul_core::app_resources::RawImageFormat;
        match f {
            RawImageFormat::R8 => crate::RawImageFormat::R8,
            RawImageFormat::RG8 => crate::RawImageFormat::RG8,
            RawImageFormat::RGB8 => crate::RawImageFormat::RGB8,
            RawImageFormat::RGBA8 => crate::RawImageFormat::RGBA8,
            RawImageFormat::R16 => crate::RawImageFormat::R16,
            RawImageFormat::RG16 => crate::RawImageFormat::RG16,
            RawImageFormat::RGB16 => crate::RawImageFormat::RGB16,
            RawImageFormat::RGBA16 => crate::RawImageFormat::RGBA16,
            RawImageFormat::BGR8 => crate::RawImageFormat::BGR8,
            RawImageFormat::BGRA8 => crate::RawImageFormat::BGRA8,
            RawImageFormat::RGBF32 => crate::RawImageFormat::RGBF32,
            RawImageFormat::RGBAF32 => crate::RawImageFormat::RGBAF32,
        }
    }

    #[cfg(feature = "html")]
    fn into_internal(&self) -> azul_core::app_resources::RawImageFormat {
        match self {
            RawImageFormat::R8 => azul_core::app_resources::RawImageFormat::R8,
            RawImageFormat::RG8 => azul_core::app_resources::RawImageFormat::RG8,
            RawImageFormat::RGB8 => azul_core::app_resources::RawImageFormat::RGB8,
            RawImageFormat::RGBA8 => azul_core::app_resources::RawImageFormat::RGBA8,
            RawImageFormat::R16 => azul_core::app_resources::RawImageFormat::R16,
            RawImageFormat::RG16 => azul_core::app_resources::RawImageFormat::RG16,
            RawImageFormat::RGB16 => azul_core::app_resources::RawImageFormat::RGB16,
            RawImageFormat::RGBA16 => azul_core::app_resources::RawImageFormat::RGBA16,
            RawImageFormat::BGR8 => azul_core::app_resources::RawImageFormat::BGR8,
            RawImageFormat::BGRA8 => azul_core::app_resources::RawImageFormat::BGRA8,
            RawImageFormat::RGBF32 => azul_core::app_resources::RawImageFormat::RGBF32,
            RawImageFormat::RGBAF32 => azul_core::app_resources::RawImageFormat::RGBAF32,
        }
    }

    pub fn has_alpha(&self) -> bool {
        use RawImageFormat::*;
        matches!(self, RGBA8 | RGBA16 | RGBAF32 | RG8 | RG16)
    }

    pub fn get_color_bits_and_space(&self) -> (ColorBits, ColorSpace) {
        use self::RawImageFormat::*;
        match self {
            R8 => (ColorBits::Bit8, ColorSpace::Greyscale),
            RG8 => (ColorBits::Bit8, ColorSpace::GreyscaleAlpha),
            RGB8 => (ColorBits::Bit8, ColorSpace::Rgb),
            RGBA8 => (ColorBits::Bit8, ColorSpace::Rgba),
            R16 => (ColorBits::Bit16, ColorSpace::Greyscale),
            RG16 => (ColorBits::Bit16, ColorSpace::GreyscaleAlpha),
            RGB16 => (ColorBits::Bit16, ColorSpace::Rgb),
            RGBA16 => (ColorBits::Bit16, ColorSpace::Rgba),
            BGR8 => (ColorBits::Bit8, ColorSpace::Rgb),
            BGRA8 => (ColorBits::Bit8, ColorSpace::Rgba),
            RGBF32 => (ColorBits::Bit16, ColorSpace::Rgb),
            RGBAF32 => (ColorBits::Bit16, ColorSpace::Rgba),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
#[serde(tag = "tag", content = "data", rename_all = "lowercase")]
pub enum RawImageData {
    // 8-bit image data
    U8(Vec<u8>),
    // 16-bit image data
    U16(Vec<u16>),
    // HDR image data
    F32(Vec<f32>),
}

impl RawImageData {
    pub fn empty(format: RawImageFormat) -> Self {
        use self::RawImageFormat::*;
        match format {
            R8 | RG8 | RGB8 | RGBA8 | BGR8 | BGRA8 => Self::U8(Vec::new()),

            R16 | RG16 | RGB16 | RGBA16 => Self::U16(Vec::new()),

            RGBF32 | RGBAF32 => Self::F32(Vec::new()),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            RawImageData::U8(vec) => vec.is_empty(),
            RawImageData::U16(vec) => vec.is_empty(),
            RawImageData::F32(vec) => vec.is_empty(),
        }
    }
}

/// Format to encode the image into
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputImageFormat {
    /// An Image in PNG Format
    Png,
    /// An Image in JPEG Format
    Jpeg,
    /// An Image in GIF Format
    Gif,
    /// An Image in WEBP Format
    Webp,
    /// An Image in general PNM Format
    Pnm,
    /// An Image in TIFF Format
    Tiff,
    /// An Image in TGA Format
    Tga,
    /// An Image in BMP Format
    Bmp,
    /// An Image in AVIF Format
    Avif,
}

impl OutputImageFormat {
    pub fn mime_type(&self) -> &'static str {
        match self {
            OutputImageFormat::Png => "image/png",
            OutputImageFormat::Jpeg => "image/jpeg",
            OutputImageFormat::Gif => "image/gif",
            OutputImageFormat::Webp => "image/webp",
            OutputImageFormat::Pnm => "image/pnm",
            OutputImageFormat::Tiff => "image/tiff",
            OutputImageFormat::Tga => "image/tga",
            OutputImageFormat::Bmp => "image/bmp",
            OutputImageFormat::Avif => "image/avif",
        }
    }
}

/// Parses a size string like "300kb" into bytes
pub fn parse_size_string(size_str: &str) -> Result<usize, String> {
    let size_str = size_str.trim().to_lowercase();
    let numeric_part: String = size_str
        .chars()
        .take_while(|c| c.is_digit(10) || *c == '.')
        .collect();
    let unit_part: String = size_str.chars().skip(numeric_part.len()).collect();

    let num = numeric_part
        .parse::<f64>()
        .map_err(|e| format!("Invalid size number: {}", e))?;

    let multiplier = match unit_part.as_str() {
        "b" => 1,
        "kb" | "k" => 1024,
        "mb" | "m" => 1024 * 1024,
        "gb" | "g" => 1024 * 1024 * 1024,
        _ => return Err(format!("Unknown size unit: {}", unit_part)),
    };

    Ok((num * multiplier as f64) as usize)
}

impl RawImage {
    /// Creates an empty `RawImage`
    pub fn empty(width: usize, height: usize, format: crate::RawImageFormat) -> Self {
        Self {
            width,
            height,
            data_format: format,
            pixels: RawImageData::empty(format),
            tag: Vec::new(),
        }
    }

    /// Same as decode_from_bytes, but uses async for browser-native image decoding
    pub async fn decode_from_bytes_async(
        bytes: &[u8],
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<Self, String> {
        // Try browser-native decoding first for better format support
        #[cfg(all(feature = "js-sys", target_family = "wasm"))]
        {
            warnings.push(PdfWarnMsg::info(
                0,
                0,
                "Attempting browser-native image decoding".to_string(),
            ));
            if let Ok(image) = browser_image::decode_image_with_browser(bytes, warnings).await {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Successfully used browser-native image decoder".to_string(),
                ));
                return Ok(image);
            }
            warnings.push(PdfWarnMsg::info(
                0,
                0,
                "Browser-native decoding failed, falling back to standard decode".to_string(),
            ));
        }

        Self::decode_from_bytes(bytes, warnings)
    }

    pub fn from_dynamic_image(image: DynamicImage) -> Result<Self, String> {
        let (w, h) = image.dimensions();
        let data_format = match image.color() {
            image::ColorType::L8 => RawImageFormat::R8,
            image::ColorType::La8 => RawImageFormat::RG8,
            image::ColorType::Rgb8 => RawImageFormat::RGB8,
            image::ColorType::Rgba8 => RawImageFormat::RGBA8,
            _ => return Err("Unsupported color type".to_string()),
        };

        let pixels = match image {
            DynamicImage::ImageLuma8(buf) => RawImageData::U8(buf.into_raw()),
            DynamicImage::ImageLumaA8(buf) => RawImageData::U8(buf.into_raw()),
            DynamicImage::ImageRgb8(buf) => RawImageData::U8(buf.into_raw()),
            DynamicImage::ImageRgba8(buf) => RawImageData::U8(buf.into_raw()),
            _ => return Err("Unsupported dynamic image format".to_string()),
        };

        Ok(RawImage {
            pixels,
            width: w as usize,
            height: h as usize,
            data_format,
            tag: Vec::new(),
        })
    }

    /// NOTE: depends on the enabled image formats!
    pub fn decode_from_bytes(bytes: &[u8], warnings: &mut Vec<PdfWarnMsg>) -> Result<Self, String> {
        use image::DynamicImage::*;

        let im = match image::guess_format(bytes) {
            Ok(format) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!("Detected image format: {:?}", format),
                ));
                format
            }
            Err(e) => return Err(e.to_string()),
        };

        let b_len = bytes.len();
        warnings.push(PdfWarnMsg::info(
            0,
            0,
            format!("Image data size: {} bytes", b_len),
        ));

        // Check feature support for various formats
        #[cfg(not(feature = "gif"))]
        {
            let err = format!(
                "cannot decode image (len = {b_len} bytes): printpdf is missing feature 'gif' to \
                 decode GIF files. Please enable it or construct the RawImage manually."
            );
            if im == image::ImageFormat::Gif {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    "GIF format detected but GIF support not compiled in".to_string(),
                ));
                return Err(err);
            }
        }

        #[cfg(not(feature = "jpeg"))]
        {
            let err = format!(
                "cannot decode image (len = {b_len} bytes): printpdf is missing feature 'jpeg' to \
                 decode JPEG files. Please enable it or construct the RawImage manually."
            );
            if im == image::ImageFormat::Jpeg {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    "JPEG format detected but JPEG support not compiled in".to_string(),
                ));
                return Err(err);
            }
        }

        #[cfg(not(feature = "png"))]
        {
            let err = format!(
                "cannot decode image (len = {b_len} bytes): printpdf is missing feature 'png' to \
                 decode PNG files. Please enable it or construct the RawImage manually."
            );
            if im == image::ImageFormat::Png {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    "PNG format detected but PNG support not compiled in".to_string(),
                ));
                return Err(err);
            }
        }

        // Check additional image formats as in the original code...

        // Decode the image
        let im = match image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
        {
            Ok(img) => img,
            Err(e) => {
                let err_msg = e.to_string();
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    format!("Image decode error: {}", err_msg),
                ));
                return Err(err_msg);
            }
        };

        let (w, h) = im.dimensions();
        warnings.push(PdfWarnMsg::info(
            0,
            0,
            format!("Image dimensions: {}x{} pixels", w, h),
        ));

        // Map the color type with informative messages
        let ct = match im.color() {
            image::ColorType::L8 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected grayscale (L8) image".to_string(),
                ));
                RawImageFormat::R8
            }
            image::ColorType::La8 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected grayscale with alpha (La8) image".to_string(),
                ));
                RawImageFormat::RG8
            }
            image::ColorType::Rgb8 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected RGB (Rgb8) image".to_string(),
                ));
                RawImageFormat::RGB8
            }
            image::ColorType::Rgba8 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected RGBA (Rgba8) image".to_string(),
                ));
                RawImageFormat::RGBA8
            }
            image::ColorType::L16 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected 16-bit grayscale (L16) image".to_string(),
                ));
                RawImageFormat::R16
            }
            image::ColorType::La16 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected 16-bit grayscale with alpha (La16) image".to_string(),
                ));
                RawImageFormat::RG16
            }
            image::ColorType::Rgb16 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected 16-bit RGB (Rgb16) image".to_string(),
                ));
                RawImageFormat::RGB16
            }
            image::ColorType::Rgba16 => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected 16-bit RGBA (Rgba16) image".to_string(),
                ));
                RawImageFormat::RGBA16
            }
            image::ColorType::Rgb32F => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected 32-bit float RGB (Rgb32F) image".to_string(),
                ));
                RawImageFormat::RGBF32
            }
            image::ColorType::Rgba32F => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Detected 32-bit float RGBA (Rgba32F) image".to_string(),
                ));
                RawImageFormat::RGBAF32
            }
            other => {
                let err_msg = format!("Unsupported color type: {:?}", other);
                warnings.push(PdfWarnMsg::warning(0, 0, err_msg.clone()));
                return Err("invalid raw image format".to_string());
            }
        };

        // Extract pixel data
        let pixels = match im {
            ImageLuma8(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageLuma8 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U8(image_buffer.into_raw())
            }
            ImageLumaA8(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageLumaA8 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U8(image_buffer.into_raw())
            }
            ImageRgb8(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageRgb8 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U8(image_buffer.into_raw())
            }
            ImageRgba8(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageRgba8 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U8(image_buffer.into_raw())
            }
            ImageLuma16(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageLuma16 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U16(image_buffer.into_raw())
            }
            ImageLumaA16(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageLumaA16 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U16(image_buffer.into_raw())
            }
            ImageRgb16(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageRgb16 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U16(image_buffer.into_raw())
            }
            ImageRgba16(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageRgba16 buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::U16(image_buffer.into_raw())
            }
            ImageRgb32F(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageRgb32F buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::F32(image_buffer.into_raw())
            }
            ImageRgba32F(image_buffer) => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!(
                        "Converting ImageRgba32F buffer of {} pixels",
                        image_buffer.len()
                    ),
                ));
                RawImageData::F32(image_buffer.into_raw())
            }
            _ => {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    "Invalid pixel format".to_string(),
                ));
                return Err("invalid pixel format".to_string());
            }
        };

        warnings.push(PdfWarnMsg::info(
            0,
            0,
            "Image decoded successfully".to_string(),
        ));

        Ok(RawImage {
            pixels,
            width: w as usize,
            height: h as usize,
            data_format: ct,
            tag: Vec::new(),
        })
    }
    pub async fn encode_to_bytes_async(
        &self,
        target_fmt: &[OutputImageFormat],
    ) -> Result<(Vec<u8>, OutputImageFormat), String> {
        #[cfg(all(feature = "js-sys", target_family = "wasm"))]
        for f in target_fmt {
            if let Ok(bytes) = browser_image::encode_image_with_browser(self, *f).await {
                return Ok((bytes, *f));
            }
        }

        self.encode_to_bytes(target_fmt)
    }
    /// NOTE: depends on the enabled image formats!
    ///
    /// Function will try to encode the image to the given formats and return an Error on
    /// exhaustion. Tries to encode the image into one of the given target formats, returning
    /// the encoded bytes if successful. For simplicity this implementation supports only 8‑bit
    /// image data.
    pub fn encode_to_bytes(
        &self,
        target_fmt: &[OutputImageFormat],
    ) -> Result<(Vec<u8>, OutputImageFormat), String> {
        // For this example we only support the U8 variant.
        let dyn_image = match (&self.pixels, self.data_format) {
            (RawImageData::U8(ref vec), RawImageFormat::R8) => {
                image::GrayImage::from_raw(self.width as u32, self.height as u32, vec.clone())
                    .map(DynamicImage::ImageLuma8)
            }
            (RawImageData::U8(ref vec), RawImageFormat::RG8) => {
                image::ImageBuffer::from_raw(self.width as u32, self.height as u32, vec.clone())
                    .map(|buf: image::ImageBuffer<image::LumaA<u8>, Vec<u8>>| {
                        DynamicImage::ImageLumaA8(buf)
                    })
            }
            (RawImageData::U8(ref vec), RawImageFormat::RGB8) => {
                image::RgbImage::from_raw(self.width as u32, self.height as u32, vec.clone())
                    .map(DynamicImage::ImageRgb8)
            }
            (RawImageData::U8(ref vec), RawImageFormat::RGBA8) => {
                image::RgbaImage::from_raw(self.width as u32, self.height as u32, vec.clone())
                    .map(DynamicImage::ImageRgba8)
            }
            _ => None,
        }
        .ok_or_else(|| {
            "Failed to construct dynamic image (unsupported pixel format?)".to_string()
        })?;

        // Try each target format in order.
        for fmt in target_fmt {
            use image::ImageFormat;
            let image_fmt = match fmt {
                OutputImageFormat::Png => ImageFormat::Png,
                OutputImageFormat::Jpeg => ImageFormat::Jpeg,
                OutputImageFormat::Gif => ImageFormat::Gif,
                OutputImageFormat::Webp => ImageFormat::WebP,
                OutputImageFormat::Pnm => ImageFormat::Pnm,
                OutputImageFormat::Tiff => ImageFormat::Tiff,
                OutputImageFormat::Tga => ImageFormat::Tga,
                OutputImageFormat::Bmp => ImageFormat::Bmp,
                OutputImageFormat::Avif => ImageFormat::Avif,
            };
            let mut buf = Vec::new();
            if dyn_image
                .write_to(&mut Cursor::new(&mut buf), image_fmt)
                .is_ok()
            {
                return Ok((buf, *fmt));
            }
        }

        Err("Could not encode image in any of the requested target formats".to_string())
    }

    /// Translates to an internal `RawImage`, necessary for the `<img>` component
    #[cfg(feature = "html")]
    pub fn to_internal(&self) -> azul_core::app_resources::ImageRef {
        let invalid = azul_core::app_resources::ImageRef::null_image(
            self.width,
            self.height,
            self.data_format.into_internal(),
            self.tag.clone(),
        );

        if self.pixels.is_empty() {
            invalid
        } else {
            azul_core::app_resources::ImageRef::new_rawimage(translate_to_internal_rawimage(self))
                .unwrap_or(invalid)
        }
    }

    /// Optimizes the image based on the provided options
    pub fn optimize(&mut self, options: &ImageOptimizationOptions) -> Result<(), String> {
        // Remove alpha channel if all pixels are opaque and auto-optimize is enabled
        if options.auto_optimize.unwrap_or_default() && self.data_format.has_alpha() {
            if self.is_fully_opaque() {
                self.remove_alpha_channel()?;
            }
        }

        // Check if color image is actually greyscale
        if options.auto_optimize.unwrap_or_default()
            && self.is_color_format()
            && self.is_actually_greyscale()
        {
            self.convert_to_greyscale()?;
        }

        // NEW: Convert to greyscale if both auto_optimize and convert_to_greyscale are true
        if options.auto_optimize.unwrap_or_default()
            && options.convert_to_greyscale.unwrap_or_default()
            && self.is_color_format()
        {
            self.convert_to_greyscale()?;
        }

        // Apply dithering to greyscale images if requested
        if options.dither_greyscale.unwrap_or_default() && self.is_greyscale_format() {
            self.apply_dithering()?;
        }

        // Resize image if it exceeds max size
        if let Some(max_size) = options
            .max_image_size
            .as_deref()
            .and_then(|s| parse_size_string(s).ok())
        {
            let current_size = self.estimate_size_bytes();
            if current_size > max_size {
                self.resize_to_fit_size(max_size)?;
            }
        }

        Ok(())
    }

    /// Checks if all pixels in the alpha channel are fully opaque
    pub fn is_fully_opaque(&self) -> bool {
        match &self.pixels {
            RawImageData::U8(data) => match self.data_format {
                RawImageFormat::RGBA8 => {
                    for i in 3..data.len() as usize {
                        if i % 4 == 3 && data[i] != 255 {
                            return false;
                        }
                    }
                    true
                }
                RawImageFormat::BGRA8 => {
                    for i in 3..data.len() as usize {
                        if i % 4 == 3 && data[i] != 255 {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },
            RawImageData::U16(data) => match self.data_format {
                RawImageFormat::RGBA16 => {
                    for i in 3..data.len() as usize {
                        if i % 4 == 3 && data[i] != 65535 {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },
            RawImageData::F32(data) => match self.data_format {
                RawImageFormat::RGBAF32 => {
                    for i in 3..data.len() as usize {
                        if i % 4 == 3 && data[i] < 0.999 {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },
        }
    }

    /// Removes the alpha channel from images that have one
    pub fn remove_alpha_channel(&mut self) -> Result<(), String> {
        self.pixels = match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGBA8) => {
                let mut rgb = Vec::with_capacity(data.len() / 4 * 3);
                for i in (0..data.len()).step_by(4) {
                    if i + 2 < data.len() {
                        rgb.push(data[i]);
                        rgb.push(data[i + 1]);
                        rgb.push(data[i + 2]);
                    }
                }
                self.data_format = RawImageFormat::RGB8;
                RawImageData::U8(rgb)
            }
            (RawImageData::U8(data), RawImageFormat::BGRA8) => {
                let mut bgr = Vec::with_capacity(data.len() / 4 * 3);
                for i in (0..data.len()).step_by(4) {
                    if i + 2 < data.len() {
                        bgr.push(data[i]);
                        bgr.push(data[i + 1]);
                        bgr.push(data[i + 2]);
                    }
                }
                self.data_format = RawImageFormat::BGR8;
                RawImageData::U8(bgr)
            }
            (RawImageData::U16(data), RawImageFormat::RGBA16) => {
                let mut rgb = Vec::with_capacity(data.len() / 4 * 3);
                for i in (0..data.len()).step_by(4) {
                    if i + 2 < data.len() {
                        rgb.push(data[i]);
                        rgb.push(data[i + 1]);
                        rgb.push(data[i + 2]);
                    }
                }
                self.data_format = RawImageFormat::RGB16;
                RawImageData::U16(rgb)
            }
            (RawImageData::F32(data), RawImageFormat::RGBAF32) => {
                let mut rgb = Vec::with_capacity(data.len() / 4 * 3);
                for i in (0..data.len()).step_by(4) {
                    if i + 2 < data.len() {
                        rgb.push(data[i]);
                        rgb.push(data[i + 1]);
                        rgb.push(data[i + 2]);
                    }
                }
                self.data_format = RawImageFormat::RGBF32;
                RawImageData::F32(rgb)
            }
            _ => return Err("Image doesn't have an alpha channel".to_string()),
        };

        Ok(())
    }

    /// Returns true if the image is in an RGB color format
    pub fn is_color_format(&self) -> bool {
        match self.data_format {
            RawImageFormat::RGB8
            | RawImageFormat::RGBA8
            | RawImageFormat::BGR8
            | RawImageFormat::BGRA8
            | RawImageFormat::RGB16
            | RawImageFormat::RGBA16
            | RawImageFormat::RGBF32
            | RawImageFormat::RGBAF32 => true,
            _ => false,
        }
    }

    /// Returns true if the image is in a greyscale format
    pub fn is_greyscale_format(&self) -> bool {
        match self.data_format {
            RawImageFormat::R8
            | RawImageFormat::RG8
            | RawImageFormat::R16
            | RawImageFormat::RG16 => true,
            _ => false,
        }
    }

    /// Checks if an RGB image actually has only greyscale content
    pub fn is_actually_greyscale(&self) -> bool {
        match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGB8)
            | (RawImageData::U8(data), RawImageFormat::BGR8) => {
                for i in (0..data.len()).step_by(3) {
                    if i + 2 < data.len() {
                        let ch1 = data[i];
                        let ch2 = data[i + 1];
                        let ch3 = data[i + 2];

                        // Allow small differences in color channels (accounting for compression
                        // artifacts)
                        if (ch1 as i16 - ch2 as i16).abs() > 3
                            || (ch1 as i16 - ch3 as i16).abs() > 3
                            || (ch2 as i16 - ch3 as i16).abs() > 3
                        {
                            return false;
                        }
                    }
                }
                true
            }
            (RawImageData::U8(data), RawImageFormat::RGBA8)
            | (RawImageData::U8(data), RawImageFormat::BGRA8) => {
                for i in (0..data.len()).step_by(4) {
                    if i + 2 < data.len() {
                        let ch1 = data[i];
                        let ch2 = data[i + 1];
                        let ch3 = data[i + 2];

                        if (ch1 as i16 - ch2 as i16).abs() > 3
                            || (ch1 as i16 - ch3 as i16).abs() > 3
                            || (ch2 as i16 - ch3 as i16).abs() > 3
                        {
                            return false;
                        }
                    }
                }
                true
            }
            (RawImageData::U16(data), RawImageFormat::RGB16)
            | (RawImageData::U16(data), RawImageFormat::RGBA16) => {
                for i in (0..data.len()).step_by(if self.data_format == RawImageFormat::RGBA16 {
                    4
                } else {
                    3
                }) {
                    if i + 2 < data.len() {
                        let ch1 = data[i];
                        let ch2 = data[i + 1];
                        let ch3 = data[i + 2];

                        // Allow slightly larger differences for 16-bit
                        if (ch1 as i32 - ch2 as i32).abs() > 768
                            || (ch1 as i32 - ch3 as i32).abs() > 768
                            || (ch2 as i32 - ch3 as i32).abs() > 768
                        {
                            return false;
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Converts a color image to greyscale
    pub fn convert_to_greyscale(&mut self) -> Result<(), String> {
        self.pixels = match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGB8) => {
                let mut grey = Vec::with_capacity(data.len() / 3);
                for i in (0..data.len()).step_by(3) {
                    if i + 2 < data.len() {
                        // Standard RGB to greyscale conversion weights
                        let g = (0.299 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.114 * data[i + 2] as f32) as u8;
                        grey.push(g);
                    }
                }
                self.data_format = RawImageFormat::R8;
                RawImageData::U8(grey)
            }
            (RawImageData::U8(data), RawImageFormat::BGR8) => {
                let mut grey = Vec::with_capacity(data.len() / 3);
                for i in (0..data.len()).step_by(3) {
                    if i + 2 < data.len() {
                        // BGR to greyscale: note the different weight order
                        let g = (0.114 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.299 * data[i + 2] as f32) as u8;
                        grey.push(g);
                    }
                }
                self.data_format = RawImageFormat::R8;
                RawImageData::U8(grey)
            }
            (RawImageData::U8(data), RawImageFormat::RGBA8) => {
                let mut grey_alpha = Vec::with_capacity(data.len() / 2);
                for i in (0..data.len()).step_by(4) {
                    if i + 3 < data.len() {
                        let g = (0.299 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.114 * data[i + 2] as f32) as u8;
                        grey_alpha.push(g); // Grayscale value
                        grey_alpha.push(data[i + 3]); // Alpha value
                    }
                }
                self.data_format = RawImageFormat::RG8; // Use RG8 instead of R8
                RawImageData::U8(grey_alpha)
            }
            (RawImageData::U8(data), RawImageFormat::BGRA8) => {
                let mut grey_alpha = Vec::with_capacity(data.len() / 2);
                for i in (0..data.len()).step_by(4) {
                    if i + 3 < data.len() {
                        // BGR to greyscale: note the different weight order
                        let g = (0.114 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.299 * data[i + 2] as f32) as u8;
                        grey_alpha.push(g); // Grayscale value
                        grey_alpha.push(data[i + 3]); // Alpha value
                    }
                }
                self.data_format = RawImageFormat::RG8;
                RawImageData::U8(grey_alpha)
            }
            (RawImageData::U16(data), RawImageFormat::RGB16) => {
                let mut grey = Vec::with_capacity(data.len() / 3);
                for i in (0..data.len()).step_by(3) {
                    if i + 2 < data.len() {
                        let g = (0.299 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.114 * data[i + 2] as f32) as u16;
                        grey.push(g);
                    }
                }
                self.data_format = RawImageFormat::R16;
                RawImageData::U16(grey)
            }
            (RawImageData::U16(data), RawImageFormat::RGBA16) => {
                let mut grey_alpha = Vec::with_capacity(data.len() / 2);
                for i in (0..data.len()).step_by(4) {
                    if i + 3 < data.len() {
                        let g = (0.299 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.114 * data[i + 2] as f32) as u16;
                        grey_alpha.push(g); // Grayscale value
                        grey_alpha.push(data[i + 3]); // Alpha value
                    }
                }
                self.data_format = RawImageFormat::RG16;
                RawImageData::U16(grey_alpha)
            }
            _ => return Err("Unsupported format for greyscale conversion".to_string()),
        };

        Ok(())
    }

    /// Applies Floyd-Steinberg dithering to a greyscale image
    pub fn apply_dithering(&mut self) -> Result<(), String> {
        if !self.is_greyscale_format() {
            return Err("Dithering can only be applied to greyscale images".to_string());
        }

        match (&mut self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::R8) => {
                // Create a mutable 2D grid for applying dithering
                let width = self.width;
                let height = self.height;
                let mut grid = Vec::with_capacity(height);

                // Convert linear data to 2D grid
                for y in 0..height {
                    let mut row = Vec::with_capacity(width);
                    for x in 0..width {
                        let idx = y * width + x;
                        if idx < data.len() {
                            row.push(data[idx] as i16);
                        } else {
                            row.push(0);
                        }
                    }
                    grid.push(row);
                }

                // Apply Floyd-Steinberg dithering
                for y in 0..height {
                    for x in 0..width {
                        let old_pixel = grid[y][x];
                        let new_pixel = if old_pixel > 127 { 255 } else { 0 };
                        let quant_error = old_pixel - new_pixel;

                        grid[y][x] = new_pixel;

                        // Distribute the error to neighboring pixels
                        if x + 1 < width {
                            grid[y][x + 1] = (grid[y][x + 1] + quant_error * 7 / 16).clamp(0, 255);
                        }

                        if y + 1 < height {
                            if x > 0 {
                                grid[y + 1][x - 1] =
                                    (grid[y + 1][x - 1] + quant_error * 3 / 16).clamp(0, 255);
                            }

                            grid[y + 1][x] = (grid[y + 1][x] + quant_error * 5 / 16).clamp(0, 255);

                            if x + 1 < width {
                                grid[y + 1][x + 1] =
                                    (grid[y + 1][x + 1] + quant_error * 1 / 16).clamp(0, 255);
                            }
                        }
                    }
                }

                // Convert back to linear data
                let mut result = Vec::with_capacity(data.len());
                for y in 0..height {
                    for x in 0..width {
                        result.push(grid[y][x] as u8);
                    }
                }

                // Update the original data
                *data = result;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::RG8) => {
                // For RG8 (grayscale+alpha), only dither the grayscale channel
                // Extract grayscale channel
                let mut grey = Vec::with_capacity(data.len() / 2);
                let mut alpha = Vec::with_capacity(data.len() / 2);

                for i in (0..data.len()).step_by(2) {
                    if i + 1 < data.len() {
                        grey.push(data[i]);
                        alpha.push(data[i + 1]);
                    }
                }

                // Create a temporary R8 image for dithering
                let mut temp_img = RawImage {
                    pixels: RawImageData::U8(grey),
                    width: self.width,
                    height: self.height,
                    data_format: RawImageFormat::R8,
                    tag: Vec::new(),
                };

                // Apply dithering to the temporary image
                temp_img.apply_dithering()?;

                // Recombine the channels
                if let RawImageData::U8(dithered_grey) = &temp_img.pixels {
                    let mut combined = Vec::with_capacity(data.len());
                    for i in 0..dithered_grey.len() {
                        if i < dithered_grey.len() && i < alpha.len() {
                            combined.push(dithered_grey[i]);
                            combined.push(alpha[i]);
                        }
                    }
                    *data = combined;
                }

                Ok(())
            }
            (RawImageData::U16(data), RawImageFormat::R16) => {
                // Dithering for 16-bit grayscale, convert to 8-bit for dithering and back to 16-bit
                let mut grey_u8 = Vec::with_capacity(data.len());

                // Convert 16-bit to 8-bit
                for &value in data.iter() {
                    grey_u8.push((value >> 8) as u8);
                }

                // Create a temporary R8 image for dithering
                let mut temp_img = RawImage {
                    pixels: RawImageData::U8(grey_u8),
                    width: self.width,
                    height: self.height,
                    data_format: RawImageFormat::R8,
                    tag: Vec::new(),
                };

                // Apply dithering to the temporary image
                temp_img.apply_dithering()?;

                // Convert back to 16-bit
                if let RawImageData::U8(dithered_grey) = &temp_img.pixels {
                    let mut result = Vec::with_capacity(data.len());
                    for &value in dithered_grey.iter() {
                        // Expand 8-bit to 16-bit
                        result.push((value as u16) << 8 | (value as u16));
                    }
                    *data = result;
                }

                Ok(())
            }
            _ => Err("Unsupported format for dithering".to_string()),
        }
    }

    /// Estimates the size of the image in bytes (uncompressed)
    pub fn estimate_size_bytes(&self) -> usize {
        let bits_per_pixel = match self.data_format {
            RawImageFormat::R8 => 8,
            RawImageFormat::RG8 => 16,
            RawImageFormat::RGB8 | RawImageFormat::BGR8 => 24,
            RawImageFormat::RGBA8 | RawImageFormat::BGRA8 => 32,
            RawImageFormat::R16 => 16,
            RawImageFormat::RG16 => 32,
            RawImageFormat::RGB16 => 48,
            RawImageFormat::RGBA16 => 64,
            RawImageFormat::RGBF32 => 96,   // 3 * 32 bits
            RawImageFormat::RGBAF32 => 128, // 4 * 32 bits
        };

        // Calculate size in bytes (rounded up to nearest byte)
        (self.width * self.height * bits_per_pixel + 7) / 8
    }

    /// Resizes the image to fit within a maximum size in bytes
    pub fn resize_to_fit_size(&mut self, max_size_bytes: usize) -> Result<(), String> {
        let current_size = self.estimate_size_bytes();
        if current_size <= max_size_bytes {
            // Already small enough
            return Ok(());
        }

        // Calculate the scaling factor needed to fit within max_size
        let scale_factor = (max_size_bytes as f64 / current_size as f64).sqrt();

        // Calculate new dimensions
        let new_width = (self.width as f64 * scale_factor).round() as usize;
        let new_height = (self.height as f64 * scale_factor).round() as usize;

        // Ensure new dimensions are at least 1 pixel
        let new_width = new_width.max(1);
        let new_height = new_height.max(1);

        match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGB8) => {
                let mut new_data = Vec::with_capacity(new_width * new_height * 3);
                for y in 0..new_height {
                    for x in 0..new_width {
                        // Map new coordinates to old coordinates (nearest neighbor)
                        let old_x = (x as f64 * self.width as f64 / new_width as f64) as usize;
                        let old_y = (y as f64 * self.height as f64 / new_height as f64) as usize;
                        let old_idx = (old_y * self.width + old_x) * 3;

                        if old_idx + 2 < data.len() {
                            new_data.push(data[old_idx]);
                            new_data.push(data[old_idx + 1]);
                            new_data.push(data[old_idx + 2]);
                        } else {
                            new_data.push(0);
                            new_data.push(0);
                            new_data.push(0);
                        }
                    }
                }
                self.pixels = RawImageData::U8(new_data);
                self.width = new_width;
                self.height = new_height;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::RGBA8) => {
                let mut new_data = Vec::with_capacity(new_width * new_height * 4);
                for y in 0..new_height {
                    for x in 0..new_width {
                        let old_x = (x as f64 * self.width as f64 / new_width as f64) as usize;
                        let old_y = (y as f64 * self.height as f64 / new_height as f64) as usize;
                        let old_idx = (old_y * self.width + old_x) * 4;

                        if old_idx + 3 < data.len() {
                            new_data.push(data[old_idx]);
                            new_data.push(data[old_idx + 1]);
                            new_data.push(data[old_idx + 2]);
                            new_data.push(data[old_idx + 3]);
                        } else {
                            new_data.push(0);
                            new_data.push(0);
                            new_data.push(0);
                            new_data.push(0);
                        }
                    }
                }
                self.pixels = RawImageData::U8(new_data);
                self.width = new_width;
                self.height = new_height;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::BGR8) => {
                let mut new_data = Vec::with_capacity(new_width * new_height * 3);
                for y in 0..new_height {
                    for x in 0..new_width {
                        let old_x = (x as f64 * self.width as f64 / new_width as f64) as usize;
                        let old_y = (y as f64 * self.height as f64 / new_height as f64) as usize;
                        let old_idx = (old_y * self.width + old_x) * 3;

                        if old_idx + 2 < data.len() {
                            new_data.push(data[old_idx]);
                            new_data.push(data[old_idx + 1]);
                            new_data.push(data[old_idx + 2]);
                        } else {
                            new_data.push(0);
                            new_data.push(0);
                            new_data.push(0);
                        }
                    }
                }
                self.pixels = RawImageData::U8(new_data);
                self.width = new_width;
                self.height = new_height;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::BGRA8) => {
                let mut new_data = Vec::with_capacity(new_width * new_height * 4);
                for y in 0..new_height {
                    for x in 0..new_width {
                        let old_x = (x as f64 * self.width as f64 / new_width as f64) as usize;
                        let old_y = (y as f64 * self.height as f64 / new_height as f64) as usize;
                        let old_idx = (old_y * self.width + old_x) * 4;

                        if old_idx + 3 < data.len() {
                            new_data.push(data[old_idx]);
                            new_data.push(data[old_idx + 1]);
                            new_data.push(data[old_idx + 2]);
                            new_data.push(data[old_idx + 3]);
                        } else {
                            new_data.push(0);
                            new_data.push(0);
                            new_data.push(0);
                            new_data.push(0);
                        }
                    }
                }
                self.pixels = RawImageData::U8(new_data);
                self.width = new_width;
                self.height = new_height;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::R8) => {
                let mut new_data = Vec::with_capacity(new_width * new_height);
                for y in 0..new_height {
                    for x in 0..new_width {
                        let old_x = (x as f64 * self.width as f64 / new_width as f64) as usize;
                        let old_y = (y as f64 * self.height as f64 / new_height as f64) as usize;
                        let old_idx = old_y * self.width + old_x;

                        if old_idx < data.len() {
                            new_data.push(data[old_idx]);
                        } else {
                            new_data.push(0);
                        }
                    }
                }
                self.pixels = RawImageData::U8(new_data);
                self.width = new_width;
                self.height = new_height;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::RG8) => {
                let mut new_data = Vec::with_capacity(new_width * new_height * 2);
                for y in 0..new_height {
                    for x in 0..new_width {
                        let old_x = (x as f64 * self.width as f64 / new_width as f64) as usize;
                        let old_y = (y as f64 * self.height as f64 / new_height as f64) as usize;
                        let old_idx = (old_y * self.width + old_x) * 2;

                        if old_idx + 1 < data.len() {
                            new_data.push(data[old_idx]);
                            new_data.push(data[old_idx + 1]);
                        } else {
                            new_data.push(0);
                            new_data.push(0);
                        }
                    }
                }
                self.pixels = RawImageData::U8(new_data);
                self.width = new_width;
                self.height = new_height;
                Ok(())
            }
            _ => Err("Resize not implemented for this format".to_string()),
        }
    }
}

pub(crate) fn image_to_stream(
    im: RawImage,
    doc: &mut lopdf::Document,
    options: Option<&ImageOptimizationOptions>,
) -> lopdf::Stream {
    use lopdf::Object::*;

    // Optimize the image if options are provided
    let mut im = im;
    if let Some(opts) = options {
        let _ = im.optimize(opts);
    }

    let (rgb8, alpha) = split_rawimage_into_rgb_plus_alpha(im);
    let (bpc, cs) = rgb8.data_format.get_color_bits_and_space();
    let interpolate = false;

    let mut dict = lopdf::Dictionary::from_iter(vec![
        ("Type", Name("XObject".into())),
        ("Subtype", Name("Image".into())),
        ("Width", Integer(rgb8.width as i64)),
        ("Height", Integer(rgb8.height as i64)),
        ("BitsPerComponent", Integer(bpc.as_integer())),
        ("ColorSpace", Name(cs.as_string().into())),
        ("Interpolate", interpolate.into()),
    ]);

    // Apply compression filter based on options
    let mut compressed_pixels = rgb8.pixels.clone();
    if let Some(opts) = options {
        if let Some(filter) = get_compression_filter(opts, &rgb8) {
            match filter {
                "DCTDecode" => {
                    // JPEG compression
                    let quality = opts.quality.unwrap_or(0.85);
                    if let Some(jpeg_data) = jpeg_encode(&rgb8, quality) {
                        compressed_pixels = jpeg_data;
                        dict.set("Filter", Name(filter.into()));
                    }
                }
                "FlateDecode" => {
                    if let Some(flate_data) = flate_encode(&rgb8.pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name(filter.into()));
                    }
                }
                "LZWDecode" => {
                    if let Some(flate_data) = flate_encode(&rgb8.pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                    /*
                    if let Some(lzw_data) = lzw_encode(&rgb8.pixels) {
                        compressed_pixels = lzw_data;
                        dict.set("Filter", Name(filter.into()));
                    }
                     */
                }
                "RunLengthDecode" => {
                    if let Some(flate_data) = flate_encode(&rgb8.pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                    /*
                    if let Some(rle_data) = rle_encode(&rgb8.pixels) {
                        compressed_pixels = rle_data;
                        dict.set("Filter", Name(filter.into()));
                    }
                    */
                }
                _ => {}
            }
        }
    }

    // Handle alpha channel (SMask)
    if let Some(alpha) = alpha {
        let mut smask_dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("XObject".into())),
            ("Subtype", Name("Image".into())),
            ("Width", Integer(rgb8.width as i64)),
            ("Height", Integer(rgb8.height as i64)),
            ("Interpolate", Boolean(false)),
            ("BitsPerComponent", Integer(ColorBits::Bit8.as_integer())),
            ("ColorSpace", Name(ColorSpace::Greyscale.as_string().into())),
        ]);

        // Alpha channel should typically use lossless compression
        let mut alpha_pixels = alpha.pixels.clone();
        let mut alpha_filter_applied = false;

        if let Some(opts) = options {
            // Get main image format or override with alpha-specific preference
            let format = opts.format.unwrap_or_default();

            // For lossy formats like JPEG, use Flate for alpha instead
            // For lossless formats, use the same compression as the main image
            let alpha_format = match format {
                ImageCompression::Auto | ImageCompression::Jpeg | ImageCompression::Jpeg2000 => {
                    ImageCompression::Flate // Use Flate for alpha by default with lossy formats
                }
                _ => format, // Use same lossless format as main image
            };

            match get_compression_filter_by_format(alpha_format) {
                "FlateDecode" => {
                    if let Some(flate_data) = flate_encode(&alpha_pixels) {
                        alpha_pixels = flate_data;
                        smask_dict.set("Filter", Name("FlateDecode".into()));
                        alpha_filter_applied = true;
                    }
                }
                "LZWDecode" => {
                    /*
                    if let Some(lzw_data) = lzw_encode(&alpha_pixels) {
                        alpha_pixels = lzw_data;
                        smask_dict.set("Filter", Name("LZWDecode".into()));
                        alpha_filter_applied = true;
                    }
                    */
                    if let Some(flate_data) = flate_encode(&rgb8.pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                }
                "RunLengthDecode" => {
                    /*
                    if let Some(rle_data) = rle_encode(&alpha_pixels) {
                        alpha_pixels = rle_data;
                        smask_dict.set("Filter", Name("RunLengthDecode".into()));
                        alpha_filter_applied = true;
                    }
                    */
                    if let Some(flate_data) = flate_encode(&rgb8.pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                }
                _ => {}
            }
        }

        // If no filter was applied yet, default to Flate
        if !alpha_filter_applied {
            if let Some(flate_data) = flate_encode(&alpha_pixels) {
                alpha_pixels = flate_data;
                smask_dict.set("Filter", Name("FlateDecode".into()));
            }
        }

        // Create stream with compression disabled (we already compressed the data)
        let mut stream = lopdf::Stream::new(smask_dict, alpha_pixels);
        stream = stream.with_compression(false);

        dict.set("SMask", Reference(doc.add_object(stream)));
    }

    // Create stream with compression disabled (we already compressed the data)
    let mut s = lopdf::Stream::new(dict, compressed_pixels);
    s = s.with_compression(false);

    s
}

// Function to select the appropriate compression filter
fn get_compression_filter(
    opts: &ImageOptimizationOptions,
    image: &RawImageU8,
) -> Option<&'static str> {
    let is_grayscale = matches!(image.data_format, RawImageFormat::R8 | RawImageFormat::RG8);
    let dithering_enabled = opts.dither_greyscale.unwrap_or_default();

    // Avoid DCT for grayscale images with dithering enabled
    if is_grayscale && dithering_enabled {
        return Some("LZWDecode"); // Use lossless compression for dithered grayscale
    }

    match opts.format.unwrap_or_default() {
        ImageCompression::Auto => {
            if matches!(image.data_format, RawImageFormat::R8 | RawImageFormat::RG8) {
                // For grayscale, LZW is often good
                Some("LZWDecode")
            } else {
                // For color images, DCT (JPEG) is usually the best choice
                #[cfg(feature = "jpeg")]
                {
                    Some("DCTDecode")
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    Some("LZWDecode")
                }
            }
        }
        ImageCompression::Jpeg | ImageCompression::Jpeg2000 => {
            #[cfg(feature = "jpeg")]
            {
                Some("DCTDecode")
            }
            #[cfg(not(feature = "jpeg"))]
            {
                Some("LZWDecode")
            }
        }
        ImageCompression::Flate => Some("FlateDecode"),
        ImageCompression::Lzw => Some("LZWDecode"),
        ImageCompression::RunLength => Some("RunLengthDecode"),
        ImageCompression::None => None,
    }
}

// Get filter string directly from format
fn get_compression_filter_by_format(format: ImageCompression) -> &'static str {
    match format {
        ImageCompression::Auto => "FlateDecode",
        ImageCompression::Jpeg | ImageCompression::Jpeg2000 => {
            #[cfg(feature = "jpeg")]
            {
                "DCTDecode"
            }
            #[cfg(not(feature = "jpeg"))]
            {
                "LZWDecode"
            }
        }
        ImageCompression::Flate => "FlateDecode",
        ImageCompression::Lzw => "LZWDecode",
        ImageCompression::RunLength => "RunLengthDecode",
        ImageCompression::None => "FlateDecode",
    }
}

// JPEG encoding using the image crate
fn jpeg_encode(image: &RawImageU8, quality: f32) -> Option<Vec<u8>> {
    let quality = (quality * 100.0) as u8;

    // Create a DynamicImage from the raw pixels
    let img = match image.data_format {
        RawImageFormat::RGB8 => image::RgbImage::from_raw(
            image.width as u32,
            image.height as u32,
            image.pixels.clone(),
        )
        .map(image::DynamicImage::ImageRgb8),
        RawImageFormat::R8 => image::GrayImage::from_raw(
            image.width as u32,
            image.height as u32,
            image.pixels.clone(),
        )
        .map(image::DynamicImage::ImageLuma8),
        RawImageFormat::BGR8 => {
            // Convert BGR to RGB
            let mut rgb_data = Vec::with_capacity(image.pixels.len());
            for chunk in image.pixels.chunks(3) {
                if chunk.len() == 3 {
                    rgb_data.push(chunk[2]); // R
                    rgb_data.push(chunk[1]); // G
                    rgb_data.push(chunk[0]); // B
                }
            }

            image::RgbImage::from_raw(image.width as u32, image.height as u32, rgb_data)
                .map(image::DynamicImage::ImageRgb8)
        }
        _ => None,
    }?;

    let mut jpeg_data = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut jpeg_data);

    // Use image crate's JPEG encoder
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    if img.write_with_encoder(encoder).is_ok() {
        Some(jpeg_data)
    } else {
        None
    }
}

// FLATE (deflate) encoding
fn flate_encode(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Write;

    use flate2::{write::ZlibEncoder, Compression};

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).ok()?;
    encoder.finish().ok()
}

// LZW encoding
#[allow(dead_code)]
fn lzw_encode(data: &[u8]) -> Option<Vec<u8>> {
    use weezl::{encode::Encoder, BitOrder};
    // Create encoder with MSB bit order (standard for PDF) and 8-bit code size
    let mut encoder = Encoder::new(BitOrder::Msb, 8);
    encoder.encode(data).ok()
}

// Simple Run Length Encoding implementation
#[allow(dead_code)]
fn rle_encode(data: &[u8]) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        let mut run_length = 1;
        let current_byte = data[i];

        // Find run length (max 128)
        while i + run_length < data.len()
            && data[i + run_length] == current_byte
            && run_length < 128
        {
            run_length += 1;
        }

        if run_length > 1 {
            // Encode run: [length-1, byte]
            result.push((run_length - 1) as u8);
            result.push(current_byte);
            i += run_length;
        } else {
            // Find literals (max 128)
            let literal_start = i;
            let mut literal_count = 1;

            i += 1;

            while i < data.len()
                && (i + 1 >= data.len() || data[i] != data[i + 1])
                && literal_count < 128
            {
                literal_count += 1;
                i += 1;
            }

            // Encode literals: [257-length, byte1, byte2, ...]
            result.push((257 - literal_count) as u8);
            for j in 0..literal_count {
                result.push(data[literal_start + j]);
            }
        }
    }

    // End of data marker
    result.push(128);

    Some(result)
}

// If the image has an alpha channel, splits the alpha channel as a separate image
// to the used in the `/Smask` dictionary
fn split_rawimage_into_rgb_plus_alpha(im: RawImage) -> (RawImageU8, Option<RawImageU8>) {
    let has_alpha = im.data_format.has_alpha();
    let (orig, alpha) = if has_alpha {
        match im.data_format {
            // Existing handling for RGBA8/BGRA8 remains:
            RawImageFormat::RGBA8 => {
                if let RawImageData::U8(vec) = im.pixels {
                    let (rgb, alpha) = rgba_to_rgb(vec);
                    (rgb, alpha)
                } else {
                    (Vec::new(), Vec::new())
                }
            }
            RawImageFormat::BGRA8 => {
                if let RawImageData::U8(vec) = im.pixels {
                    let (bgr, alpha) = bgra_to_bgr(vec);
                    (bgr, alpha)
                } else {
                    (Vec::new(), Vec::new())
                }
            }
            RawImageFormat::RG8 => {
                if let RawImageData::U8(vec) = im.pixels {
                    let mut grey = Vec::with_capacity(vec.len() / 2);
                    let mut alpha = Vec::with_capacity(vec.len() / 2);
                    for i in (0..vec.len()).step_by(2) {
                        grey.push(vec[i]);
                        alpha.push(vec[i + 1]);
                    }
                    (grey, alpha)
                } else {
                    (Vec::new(), Vec::new())
                }
            }
            RawImageFormat::RG16 => {
                if let RawImageData::U16(vec) = im.pixels {
                    let mut grey = Vec::with_capacity(vec.len() / 2);
                    let mut alpha = Vec::with_capacity(vec.len() / 2);
                    for i in (0..vec.len()).step_by(2) {
                        grey.push(vec[i]);
                        alpha.push(vec[i + 1]);
                    }
                    (u16vec_to_u8(grey), u16vec_to_u8(alpha))
                } else {
                    (Vec::new(), Vec::new())
                }
            }
            _ => (Vec::new(), Vec::new()),
        }
    } else {
        match im.pixels {
            RawImageData::U8(vec) => (vec, Vec::new()),
            RawImageData::U16(vec) => (u16vec_to_u8(vec), Vec::new()),
            RawImageData::F32(vec) => (f32vec_to_u8(vec), Vec::new()),
        }
    };

    let orig = RawImageU8 {
        pixels: orig,
        width: im.width,
        height: im.height,
        data_format: im.data_format.reduce_to_rgb(),
    };

    let alpha_mask = if alpha.is_empty() {
        None
    } else {
        Some(RawImageU8 {
            pixels: alpha,
            width: im.width,
            height: im.height,
            data_format: RawImageFormat::R8,
        })
    };

    (orig, alpha_mask)
}

// Helper function to extract alpha channel from BGRA
fn bgra_to_bgr(rgba: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(rgba.len() / 4);

    for i in (0..rgba.len()).step_by(4) {
        if i + 3 < rgba.len() {
            rgb.push(rgba[i]); // B
            rgb.push(rgba[i + 1]); // G
            rgb.push(rgba[i + 2]); // R
            alpha.push(rgba[i + 3]); // A
        }
    }

    (rgb, alpha)
}

#[cfg(feature = "html")]
pub fn translate_to_internal_rawimage(im: &RawImage) -> azul_core::app_resources::RawImage {
    azul_core::app_resources::RawImage {
        pixels: match &im.pixels {
            RawImageData::U8(vec) => azul_core::app_resources::RawImageData::U8(vec.clone().into()),
            RawImageData::U16(vec) => {
                azul_core::app_resources::RawImageData::U16(vec.clone().into())
            }
            RawImageData::F32(vec) => {
                azul_core::app_resources::RawImageData::F32(vec.clone().into())
            }
        },
        width: im.width,
        height: im.height,
        premultiplied_alpha: false,
        data_format: im.data_format.into_internal(),
        tag: im.tag.clone().into(),
    }
}

/// Takes a Vec<u8> of RGBA data and returns two Vec<u8> of RGB and alpha data
fn rgba_to_rgb(data: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let mut rgb = Vec::with_capacity(data.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(data.len() / 4);
    for i in (0..data.len()).step_by(4) {
        rgb.push(data[i]);
        rgb.push(data[i + 1]);
        rgb.push(data[i + 2]);
        alpha.push(data[i + 3]);
    }

    (rgb, alpha)
}

#[allow(dead_code)]
fn rgba_to_rgb16(data: Vec<u16>) -> (Vec<u16>, Vec<u16>) {
    let mut rgb = Vec::with_capacity(data.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(data.len() / 4);
    for i in (0..data.len()).step_by(4) {
        rgb.push(data[i]);
        rgb.push(data[i + 1]);
        rgb.push(data[i + 2]);
        alpha.push(data[i + 3]);
    }

    (rgb, alpha)
}

#[allow(dead_code)]
fn rgba_to_rgbf32(data: Vec<f32>) -> (Vec<f32>, Vec<f32>) {
    let mut rgb = Vec::with_capacity(data.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(data.len() / 4);
    for i in (0..data.len()).step_by(4) {
        rgb.push(data[i]);
        rgb.push(data[i + 1]);
        rgb.push(data[i + 2]);
        alpha.push(data[i + 3]);
    }

    (rgb, alpha)
}

fn u16vec_to_u8(data: Vec<u16>) -> Vec<u8> {
    data.iter().flat_map(|us| us.to_be_bytes()).collect()
}

fn f32vec_to_u8(data: Vec<f32>) -> Vec<u8> {
    data.iter().flat_map(|us| us.to_be_bytes()).collect()
}

#[cfg(all(feature = "js-sys", target_family = "wasm"))]
mod browser_image {

    use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        js_sys, window, Blob, BlobPropertyBag, CanvasRenderingContext2d, HtmlCanvasElement,
        HtmlImageElement, ImageBitmap,
    };

    use super::{OutputImageFormat, RawImage, RawImageData, RawImageFormat};
    use crate::PdfWarnMsg;

    // Decode image bytes using browser's capabilities
    pub async fn decode_image_with_browser(
        bytes: &[u8],
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<RawImage, String> {
        let window = window().ok_or("No window available")?;

        // Create a Blob from the bytes
        let array = Array::new();
        let uint8_array = Uint8Array::from(bytes);
        array.push(&uint8_array.buffer());

        let options = BlobPropertyBag::new();
        let blob = Blob::new_with_u8_array_sequence_and_options(&array, &options)
            .map_err(|e| format!("Failed to create Blob: {:?}", e))?;

        // Create ImageBitmap from Blob
        let promise = window
            .create_image_bitmap_with_blob(&blob)
            .map_err(|e| format!("Failed to create ImageBitmap: {:?}", e))?;

        let bitmap: ImageBitmap = JsFuture::from(promise)
            .await
            .map_err(|e| format!("Promise rejected: {:?}", e))?
            .dyn_into()
            .map_err(|_| "Failed to cast to ImageBitmap")?;

        // Create a canvas to extract pixel data
        let document = window.document().ok_or("No document available")?;
        let canvas = document
            .create_element("canvas")
            .map_err(|_| "Failed to create canvas")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Failed to cast to HtmlCanvasElement")?;

        let width = bitmap.width() as usize;
        let height = bitmap.height() as usize;

        canvas.set_width(width as u32);
        canvas.set_height(height as u32);

        let context = canvas
            .get_context("2d")
            .map_err(|_| "Failed to get context")?
            .ok_or("Context is null")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Failed to cast to CanvasRenderingContext2d")?;

        context
            .draw_image_with_image_bitmap(&bitmap, 0.0, 0.0)
            .map_err(|_| "Failed to draw image")?;

        let image_data = context
            .get_image_data(0.0, 0.0, width as f64, height as f64)
            .map_err(|_| "Failed to get image data")?;

        let data = image_data.data();
        let data_vec = data.to_vec();

        // Convert to RawImage (RGBA8 format)
        Ok(RawImage {
            pixels: RawImageData::U8(data_vec),
            width,
            height,
            data_format: RawImageFormat::RGBA8,
            tag: Vec::new(),
        })
    }

    // Encode image to specific format using browser's Canvas API
    pub async fn encode_image_with_browser(
        image: &RawImage,
        format: OutputImageFormat,
    ) -> Result<Vec<u8>, String> {
        // Convert format to mime type
        let mime_type = match format {
            OutputImageFormat::Jpeg => "image/jpeg",
            OutputImageFormat::Png => "image/png",
            OutputImageFormat::Webp => "image/webp",
            _ => return Err(format!("Format {:?} not supported by browser", format)),
        };

        // Get pixel data
        let pixels = match &image.pixels {
            RawImageData::U8(data) => data,
            _ => return Err("Only U8 data is supported for browser encoding".to_string()),
        };

        // Set up canvas
        let window = window().ok_or("No window available")?;
        let document = window.document().ok_or("No document available")?;
        let canvas = document
            .create_element("canvas")
            .map_err(|_| "Failed to create canvas")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Failed to cast to HtmlCanvasElement")?;

        canvas.set_width(image.width as u32);
        canvas.set_height(image.height as u32);

        let context = canvas
            .get_context("2d")
            .map_err(|_| "Failed to get context")?
            .ok_or("Context is null")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Failed to cast to CanvasRenderingContext2d")?;

        // Create ImageData and put it on canvas
        let uint8_clamped_array = js_sys::Uint8ClampedArray::from(pixels.as_slice());
        let image_data = web_sys::ImageData::new_with_js_u8_clamped_array_and_sh(
            &uint8_clamped_array,
            image.width as u32,
            image.height as u32,
        )
        .map_err(|_| "Failed to create ImageData")?;

        context
            .put_image_data(&image_data, 0.0, 0.0)
            .map_err(|_| "Failed to put image data")?;

        // Get data URL
        let data_url = canvas
            .to_data_url_with_type(mime_type)
            .map_err(|_| format!("Failed to encode to {}", mime_type))?;

        // Extract binary data from data URL
        let bytes = crate::Base64OrRaw::B64(data_url).decode_bytes()?;

        Ok(bytes)
    }
}
