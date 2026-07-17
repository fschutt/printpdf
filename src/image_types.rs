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
#[serde(try_from = "RawImageDataWire", into = "RawImageDataWire")]
pub enum RawImageData {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

/// Wire format for [`RawImageData`].
///
/// Pixels serialize as base64 (`{"u8b64": "..."}`), NOT as JSON number arrays:
/// a 1024×1024 RGBA image is 4.2 million array elements ≈ 16 MB of JSON, and
/// the wasm demo shuttles that through serde + `JSON.parse` on every render —
/// the sign-pdf tab spent 90+ seconds on a single signature decode. Base64 is
/// ~4× smaller and an order of magnitude faster on both sides. The legacy
/// array variants (`u8`/`u16`/`f32`, the pre-0.12 format) stay accepted on
/// input forever; `u16`/`f32` pack little-endian.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawImageDataWire {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
    U8B64(String),
    U16B64(String),
    F32B64(String),
}

impl From<RawImageData> for RawImageDataWire {
    fn from(d: RawImageData) -> Self {
        use base64::Engine;
        let b64 = |bytes: &[u8]| base64::prelude::BASE64_STANDARD.encode(bytes);
        match d {
            RawImageData::U8(v) => RawImageDataWire::U8B64(b64(&v)),
            RawImageData::U16(v) => {
                let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                RawImageDataWire::U16B64(b64(&bytes))
            }
            RawImageData::F32(v) => {
                let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                RawImageDataWire::F32B64(b64(&bytes))
            }
        }
    }
}

impl TryFrom<RawImageDataWire> for RawImageData {
    type Error = String;
    fn try_from(w: RawImageDataWire) -> Result<Self, String> {
        use base64::Engine;
        let dec = |s: &str| {
            base64::prelude::BASE64_STANDARD
                .decode(s)
                .map_err(|e| format!("pixel data base64: {e}"))
        };
        Ok(match w {
            RawImageDataWire::U8(v) => RawImageData::U8(v),
            RawImageDataWire::U16(v) => RawImageData::U16(v),
            RawImageDataWire::F32(v) => RawImageData::F32(v),
            RawImageDataWire::U8B64(s) => RawImageData::U8(dec(&s)?),
            RawImageDataWire::U16B64(s) => {
                let b = dec(&s)?;
                if b.len() % 2 != 0 {
                    return Err("u16b64 pixel data has odd byte length".to_string());
                }
                RawImageData::U16(
                    b.chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect(),
                )
            }
            RawImageDataWire::F32B64(s) => {
                let b = dec(&s)?;
                if b.len() % 4 != 0 {
                    return Err("f32b64 pixel data length not a multiple of 4".to_string());
                }
                RawImageData::F32(
                    b.chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect(),
                )
            }
        })
    }
}

#[cfg(test)]
mod pixel_wire_tests {
    use super::*;

    #[test]
    fn pixels_serialize_as_base64_and_accept_legacy_arrays() {
        let d = RawImageData::U8(vec![1, 2, 3, 255]);
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("u8b64"), "new format must be base64: {json}");
        assert_eq!(serde_json::from_str::<RawImageData>(&json).unwrap(), d);
        // pre-0.12 wire format stays readable
        let legacy: RawImageData = serde_json::from_str(r#"{"u8":[1,2,3,255]}"#).unwrap();
        assert_eq!(legacy, d);

        let d16 = RawImageData::U16(vec![0x0102, 0xFFEE]);
        let json16 = serde_json::to_string(&d16).unwrap();
        assert_eq!(serde_json::from_str::<RawImageData>(&json16).unwrap(), d16);
    }
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

    /// Encode to bytes - returns placeholder when 'images' feature is not enabled
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

/// Encode a `RawImage` into a PDF image XObject stream using no image codec at
/// all — raw pixels + FlateDecode (flate2 is an unconditional dependency).
///
/// This exists so documents containing images can still be SAVED when the
/// `images` feature is off: `add_xobject_to_document` used to `panic!` in that
/// configuration, which made every parsed-then-resaved document with a bitmap a
/// crash. Alpha channels are split into a grayscale `/SMask` per the PDF spec;
/// 16-bit and float data is downconverted to 8-bit (a fidelity loss, but a
/// valid file — the `images` feature path keeps full depth).
pub(crate) fn raw_image_to_basic_stream(
    im: &RawImage,
    doc: &mut lopdf::Document,
) -> lopdf::Stream {
    use lopdf::Object::{Integer, Name, Reference};

    fn to_u8(px: &RawImageData) -> Vec<u8> {
        match px {
            RawImageData::U8(v) => v.clone(),
            RawImageData::U16(v) => v.iter().map(|x| (x >> 8) as u8).collect(),
            RawImageData::F32(v) => v
                .iter()
                .map(|x| (x.clamp(0.0, 1.0) * 255.0) as u8)
                .collect(),
        }
    }

    let raw = to_u8(&im.pixels);

    // (color bytes per pixel, alpha present, needs BGR swizzle, colorspace)
    let (ncolor, has_alpha, bgr, cs) = match im.data_format {
        RawImageFormat::R8 | RawImageFormat::R16 => (1, false, false, "DeviceGray"),
        RawImageFormat::RG8 | RawImageFormat::RG16 => (1, true, false, "DeviceGray"),
        RawImageFormat::RGB8 | RawImageFormat::RGB16 | RawImageFormat::RGBF32 => {
            (3, false, false, "DeviceRGB")
        }
        RawImageFormat::RGBA8 | RawImageFormat::RGBA16 | RawImageFormat::RGBAF32 => {
            (3, true, false, "DeviceRGB")
        }
        RawImageFormat::BGR8 => (3, false, true, "DeviceRGB"),
        RawImageFormat::BGRA8 => (3, true, true, "DeviceRGB"),
    };

    let stride = ncolor + usize::from(has_alpha);
    let mut color = Vec::with_capacity(im.width * im.height * ncolor);
    let mut alpha = if has_alpha {
        Vec::with_capacity(im.width * im.height)
    } else {
        Vec::new()
    };
    for px in raw.chunks_exact(stride) {
        match (ncolor, bgr) {
            (3, false) => color.extend_from_slice(&px[0..3]),
            (3, true) => color.extend_from_slice(&[px[2], px[1], px[0]]),
            _ => color.push(px[0]),
        }
        if has_alpha {
            alpha.push(px[stride - 1]);
        }
    }

    fn flate(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut e =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let _ = e.write_all(data);
        e.finish().unwrap_or_default()
    }

    let mut dict = lopdf::Dictionary::from_iter(vec![
        ("Type", Name("XObject".into())),
        ("Subtype", Name("Image".into())),
        ("Width", Integer(im.width as i64)),
        ("Height", Integer(im.height as i64)),
        ("BitsPerComponent", Integer(8)),
        ("ColorSpace", Name(cs.into())),
        ("Filter", Name("FlateDecode".into())),
    ]);

    if has_alpha {
        let smask_dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("XObject".into())),
            ("Subtype", Name("Image".into())),
            ("Width", Integer(im.width as i64)),
            ("Height", Integer(im.height as i64)),
            ("BitsPerComponent", Integer(8)),
            ("ColorSpace", Name("DeviceGray".into())),
            ("Filter", Name("FlateDecode".into())),
        ]);
        let smask_id = doc.add_object(lopdf::Stream::new(smask_dict, flate(&alpha)));
        dict.set("SMask", Reference(smask_id));
    }

    lopdf::Stream::new(dict, flate(&color)).with_compression(false)
}
