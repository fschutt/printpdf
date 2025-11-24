// Image-related types that are always available, regardless of the 'images' feature
use serde_derive::{Deserialize, Serialize};

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputImageFormat {
    /// PNG (lossless)
    Png,
    /// JPEG (lossy)
    Jpeg,
    /// WebP (lossy or lossless)
    Webp,
    /// AVIF (lossy or lossless)
    Avif,
    /// GIF (lossless, limited colors)
    Gif,
    /// BMP (lossless, uncompressed)
    Bmp,
    /// TIFF (lossless or lossy)
    Tiff,
    /// TGA (lossless)
    Tga,
    /// PNM (lossless)
    Pnm,
}

impl OutputImageFormat {
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Avif => "image/avif",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
            Self::Tga => "image/x-tga",
            Self::Pnm => "image/x-portable-anymap",
        }
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RawImageData {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

/// Raw image data container (always available, even without 'images' feature)
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RawImage {
    pub pixels: RawImageData,
    pub width: usize,
    pub height: usize,
    pub data_format: RawImageFormat,
    pub tag: Vec<u8>,
}

impl RawImage {
    /// Stub decode when 'images' feature is not enabled
    #[cfg(not(feature = "images"))]
    pub fn decode_from_bytes(_bytes: &[u8], _warnings: &mut Vec<crate::PdfWarnMsg>) -> Result<Self, String> {
        Err("RawImage::decode_from_bytes requires the 'images' feature".to_string())
    }

    /// Stub decode_async when 'images' feature is not enabled
    #[cfg(not(feature = "images"))]
    pub async fn decode_from_bytes_async(_bytes: &[u8], _warnings: &mut Vec<crate::PdfWarnMsg>) -> Result<Self, String> {
        Err("RawImage::decode_from_bytes_async requires the 'images' feature".to_string())
    }

    /// Encode to bytes - returns placeholder SVG data when 'images' feature is not enabled
    #[cfg(not(feature = "images"))]
    pub fn encode_to_bytes(&self, formats: &[OutputImageFormat]) -> Result<(Vec<u8>, OutputImageFormat), String> {
        // Return 1x1 transparent PNG as placeholder
        let png_data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 dimensions
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89,
            0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
            0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND chunk
            0xAE, 0x42, 0x60, 0x82,
        ];
        let format = formats.first().copied().unwrap_or(OutputImageFormat::Png);
        Ok((png_data, format))
    }

    /// Encode to bytes async - returns placeholder when 'images' feature is not enabled
    #[cfg(not(feature = "images"))]
    pub async fn encode_to_bytes_async(&self, formats: &[OutputImageFormat]) -> Result<(Vec<u8>, OutputImageFormat), String> {
        self.encode_to_bytes(formats)
    }
}
