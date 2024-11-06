use std::io::Cursor;
use image::{EncodableLayout, GenericImageView};
use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub struct RawImage {
    pub pixels: RawImageData,
    pub width: usize,
    pub height: usize,
    pub premultiplied_alpha: bool,
    pub data_format: RawImageFormat,
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

impl RawImage {
    /// NOTE: depends on the enabled image formats!
    pub fn decode_from_bytes(bytes: &[u8]) -> Result<Self, String> {
        use image::DynamicImage::*;

        let im = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;

        let (w, h) = im.dimensions();
        let ct = match im.color() {
            image::ColorType::L8 => RawImageFormat::R8,
            image::ColorType::La8 => RawImageFormat::RG8,
            image::ColorType::Rgb8 => RawImageFormat::RGB8,
            image::ColorType::Rgba8 => RawImageFormat::RGBA8,
            image::ColorType::L16 => RawImageFormat::R16,
            image::ColorType::La16 => RawImageFormat::RG16,
            image::ColorType::Rgb16 => RawImageFormat::RGB16,
            image::ColorType::Rgba16 => RawImageFormat::RGBA16,
            image::ColorType::Rgb32F => RawImageFormat::RGBF32,
            image::ColorType::Rgba32F => RawImageFormat::RGBAF32,
            _ => return Err(format!("invalid raw image format")),
        };
        
        let pixels = match im {
            ImageLuma8(image_buffer) => RawImageData::U8(image_buffer.into_raw()),
            ImageLumaA8(image_buffer) => RawImageData::U8(image_buffer.into_raw()),
            ImageRgb8(image_buffer) => RawImageData::U8(image_buffer.into_raw()),
            ImageRgba8(image_buffer) => RawImageData::U8(image_buffer.into_raw()),
            ImageLuma16(image_buffer) => RawImageData::U16(image_buffer.into_raw()),
            ImageLumaA16(image_buffer) => RawImageData::U16(image_buffer.into_raw()),
            ImageRgb16(image_buffer) => RawImageData::U16(image_buffer.into_raw()),
            ImageRgba16(image_buffer) => RawImageData::U16(image_buffer.into_raw()),
            ImageRgb32F(image_buffer) => RawImageData::F32(image_buffer.into_raw()),
            ImageRgba32F(image_buffer) =>RawImageData::F32(image_buffer.into_raw()),
            _ => return Err(format!("invalid pixel format")),
        };

        Ok(RawImage {
            pixels,
            width: w as usize,
            height: h as usize,
            premultiplied_alpha: false,
            data_format: ct,
        })
    }
}

pub(crate) fn translate_to_internal_rawimage(
    im: &RawImage
) -> azul_core::app_resources::RawImage {
    azul_core::app_resources::RawImage {
        pixels: match &im.pixels {
            RawImageData::U8(vec) => azul_core::app_resources::RawImageData::U8(vec.clone().into()),
            RawImageData::U16(vec) => azul_core::app_resources::RawImageData::U16(vec.clone().into()),
            RawImageData::F32(vec) => azul_core::app_resources::RawImageData::F32(vec.clone().into()),
        },
        width: im.width,
        height: im.height,
        premultiplied_alpha: im.premultiplied_alpha,
        data_format: match &im.data_format {
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
        },
    }
}
