//! Decoding and encoding image files. Requires the `images` feature.
//!
//! It also turns a [`RawImage`] into a PDF image stream. The file formats need their own features
//! (`png`, `jpeg`, `gif`, ...). The types and the explanation of how PDF stores images are in
//! [`image_types`](crate::image_types).
//!
//! [`RawImage::decode_from_bytes`] reads a file, [`RawImage::encode_to_bytes`] writes one. The
//! other methods change the pixels (alpha, greyscale, dithering, size) and are what
//! [`ImageOptimizationOptions`] applies when a document is saved.

use core::fmt;
use std::io::Cursor;

use image::{DynamicImage, GenericImageView};

use crate::{ColorBits, ColorSpace, PdfWarnMsg};

pub use crate::image_types::{
    ImageCompression, ImageOptimizationOptions, OutputImageFormat, RawImage, RawImageData,
    RawImageFormat,
};

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

impl RawImageFormat {
    /// The same format without the alpha channel: RGBA becomes RGB, grey and alpha (`RG`)
    /// becomes grey (`R`). Other formats are returned as they are.
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

    /// Whether the format has an alpha channel (RGBA, BGRA, and grey with alpha).
    pub fn has_alpha(&self) -> bool {
        use RawImageFormat::*;
        matches!(self, RGBA8 | RGBA16 | RGBAF32 | RG8 | RG16)
    }

    /// Bits per sample and color space of the format. BGR counts as RGB, and the float formats
    /// are reported as 16 bits.
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

impl RawImageData {
    /// An empty buffer of the sample type that `format` uses.
    pub fn empty(format: RawImageFormat) -> Self {
        use self::RawImageFormat::*;
        match format {
            R8 | RG8 | RGB8 | RGBA8 | BGR8 | BGRA8 => Self::U8(Vec::new()),

            R16 | RG16 | RGB16 | RGBA16 => Self::U16(Vec::new()),

            RGBF32 | RGBAF32 => Self::F32(Vec::new()),
        }
    }

    /// Whether the buffer holds no samples.
    pub fn is_empty(&self) -> bool {
        match self {
            RawImageData::U8(vec) => vec.is_empty(),
            RawImageData::U16(vec) => vec.is_empty(),
            RawImageData::F32(vec) => vec.is_empty(),
        }
    }
}

/// Parses a size such as `"300kb"` or `"1.5MB"` into bytes.
///
/// Units are `b`, `kb` (or `k`), `mb` (or `m`) and `gb` (or `g`), case-insensitive, 1024 based.
/// A number without a unit is an error.
///
/// # Errors
///
/// Returns a message when the number does not parse or the unit is unknown.
pub fn parse_size_string(size_str: &str) -> Result<usize, String> {
    let size_str = size_str.trim().to_lowercase();
    let numeric_part: String = size_str
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
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
    /// Creates an image of the given size in pixels with an empty pixel buffer.
    pub fn empty(width: usize, height: usize, format: crate::RawImageFormat) -> Self {
        Self {
            width,
            height,
            data_format: format,
            pixels: RawImageData::empty(format),
            tag: Vec::new(),
        }
    }

    /// Like [`RawImage::decode_from_bytes`]. In a browser (`wasm32` with the `js-sys` feature) it
    /// tries the browser's own decoder first, which reads more formats, and falls back to
    /// [`RawImage::decode_from_bytes`].
    pub async fn decode_from_bytes_async(
        bytes: &[u8],
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<Self, String> {
        // WASI (p1/p2) is excluded: the browser path would import `window` and canvas functions
        // that no WASI runtime provides.
        #[cfg(all(
            feature = "js-sys",
            target_arch = "wasm32",
            not(any(target_env = "p1", target_env = "p2"))
        ))]
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

    /// Converts an 8-bit grey, grey-alpha, RGB or RGBA image of the `image` crate.
    ///
    /// # Errors
    ///
    /// Returns a message for 16-bit and float images.
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

    /// Decodes an image file (the format is detected from the bytes) into pixels.
    ///
    /// The result is 8-bit, 16-bit or float, with or without alpha, as the file has it.
    /// `warnings` receives info messages (page 0, operation 0) about the detected format and
    /// size, and a warning when the decoder fails.
    ///
    /// Requires the feature of the file format (`png`, `jpeg`, `gif`, `bmp`, `tiff`, `webp`, ...).
    ///
    /// # Errors
    ///
    /// Returns a message when the format is not recognized or its feature is not enabled, when
    /// decoding fails, or for a color type that has no [`RawImageFormat`].
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "png")] {
    /// use printpdf::*;
    ///
    /// let image = RawImage {
    ///     pixels: RawImageData::U8(vec![255, 0, 0, 0, 255, 0]),
    ///     width: 2,
    ///     height: 1,
    ///     data_format: RawImageFormat::RGB8,
    ///     tag: Vec::new(),
    /// };
    /// // Normally these bytes come from a file: std::fs::read("picture.png")
    /// let (png, _) = image.encode_to_bytes(&[OutputImageFormat::Png]).unwrap();
    /// let decoded = RawImage::decode_from_bytes(&png, &mut Vec::new()).unwrap();
    /// assert_eq!((decoded.width, decoded.height), (2, 1));
    /// # }
    /// ```
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

    /// Like [`RawImage::encode_to_bytes`]. In a browser (`wasm32` with the `js-sys` feature) it
    /// tries the browser's encoder first.
    pub async fn encode_to_bytes_async(
        &self,
        target_fmt: &[OutputImageFormat],
    ) -> Result<(Vec<u8>, OutputImageFormat), String> {
        #[cfg(all(
            feature = "js-sys",
            target_arch = "wasm32",
            not(any(target_env = "p1", target_env = "p2"))
        ))]
        for f in target_fmt {
            if let Ok(bytes) = browser_image::encode_image_with_browser(self, *f).await {
                return Ok((bytes, *f));
            }
        }

        self.encode_to_bytes(target_fmt)
    }

    /// Encodes the image as a file in the first of `target_fmt` that works, and returns the
    /// bytes and the format used.
    ///
    /// Only 8-bit `R8`, `RG8`, `RGB8` and `RGBA8` images can be encoded. A format only works
    /// when its feature is enabled (`png`, `jpeg`, ...).
    ///
    /// # Errors
    ///
    /// Returns a message for any other pixel format and when no format in `target_fmt` can be
    /// written.
    pub fn encode_to_bytes(
        &self,
        target_fmt: &[OutputImageFormat],
    ) -> Result<(Vec<u8>, OutputImageFormat), String> {
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

    /// Applies the pixel changes of [`ImageOptimizationOptions`], in this order, each only if
    /// the options ask for it: drop an alpha channel that is fully opaque (`auto_optimize`),
    /// convert color to greyscale if it is nearly grey (`auto_optimize`) or if
    /// `convert_to_greyscale` is set, scale down to `max_image_size`, then dither
    /// (`dither_greyscale`). All but the first change the picture. `quality` and `format` are
    /// used later, when the image is encoded.
    ///
    /// # Errors
    ///
    /// Returns the message of the step that failed, for instance when the image is too large
    /// but its format cannot be scaled (see [`RawImage::resize_to_fit_size`]).
    pub fn optimize(&mut self, options: &ImageOptimizationOptions) -> Result<(), String> {
        if options.auto_optimize.unwrap_or_default()
            && self.data_format.has_alpha()
            && self.is_fully_opaque()
        {
            self.remove_alpha_channel()?;
        }

        if options.auto_optimize.unwrap_or_default()
            && self.is_color_format()
            && self.is_actually_greyscale()
        {
            self.convert_to_greyscale()?;
        }

        if options.auto_optimize.unwrap_or_default()
            && options.convert_to_greyscale.unwrap_or_default()
            && self.is_color_format()
        {
            self.convert_to_greyscale()?;
        }

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

        // Last: scaling a dithered image down keeps every n-th pixel, which no longer has the
        // tone of the picture.
        if options.dither_greyscale.unwrap_or_default() && self.is_greyscale_format() {
            self.apply_dithering()?;
        }

        Ok(())
    }

    /// Whether every pixel is fully opaque. False for formats without alpha, and for the
    /// grey-and-alpha formats.
    pub fn is_fully_opaque(&self) -> bool {
        match &self.pixels {
            RawImageData::U8(data) => match self.data_format {
                RawImageFormat::RGBA8 => {
                    for i in 3..data.len() {
                        if i % 4 == 3 && data[i] != 255 {
                            return false;
                        }
                    }
                    true
                }
                RawImageFormat::BGRA8 => {
                    for i in 3..data.len() {
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
                    for i in 3..data.len() {
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
                    for i in 3..data.len() {
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

    /// Drops the alpha channel of an `RGBA8`, `BGRA8`, `RGBA16` or `RGBAF32` image.
    ///
    /// # Errors
    ///
    /// Returns a message for every other format, including grey with alpha.
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

    /// Whether the format has red, green and blue channels (in either order).
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

    /// Whether the format is grey, with or without alpha.
    pub fn is_greyscale_format(&self) -> bool {
        match self.data_format {
            RawImageFormat::R8
            | RawImageFormat::RG8
            | RawImageFormat::R16
            | RawImageFormat::RG16 => true,
            _ => false,
        }
    }

    /// Whether the color channels of an 8-bit or 16-bit RGB image are nearly equal everywhere
    /// (at most 3 of 255, or 768 of 65535, apart), so it could be stored as greyscale. False
    /// for the float formats.
    pub fn is_actually_greyscale(&self) -> bool {
        match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGB8)
            | (RawImageData::U8(data), RawImageFormat::BGR8) => {
                for i in (0..data.len()).step_by(3) {
                    if i + 2 < data.len() {
                        let ch1 = data[i];
                        let ch2 = data[i + 1];
                        let ch3 = data[i + 2];

                        // a small difference is tolerated, as JPEG artifacts cause it
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

    /// Converts a color image to greyscale with the luma weights 0.299, 0.587 and 0.114. Alpha
    /// is kept: `RGBA8` and `BGRA8` become `RG8`, `RGBA16` becomes `RG16`.
    ///
    /// # Errors
    ///
    /// Returns a message for greyscale and float images.
    pub fn convert_to_greyscale(&mut self) -> Result<(), String> {
        self.pixels = match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGB8) => {
                let mut grey = Vec::with_capacity(data.len() / 3);
                for i in (0..data.len()).step_by(3) {
                    if i + 2 < data.len() {
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
                        grey_alpha.push(g);
                        grey_alpha.push(data[i + 3]);
                    }
                }
                self.data_format = RawImageFormat::RG8;
                RawImageData::U8(grey_alpha)
            }
            (RawImageData::U8(data), RawImageFormat::BGRA8) => {
                let mut grey_alpha = Vec::with_capacity(data.len() / 2);
                for i in (0..data.len()).step_by(4) {
                    if i + 3 < data.len() {
                        let g = (0.114 * data[i] as f32
                            + 0.587 * data[i + 1] as f32
                            + 0.299 * data[i + 2] as f32) as u8;
                        grey_alpha.push(g);
                        grey_alpha.push(data[i + 3]);
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
                        grey_alpha.push(g);
                        grey_alpha.push(data[i + 3]);
                    }
                }
                self.data_format = RawImageFormat::RG16;
                RawImageData::U16(grey_alpha)
            }
            _ => return Err("Unsupported format for greyscale conversion".to_string()),
        };

        Ok(())
    }

    /// Turns a greyscale image into pure black and white (threshold 127) and spreads the error
    /// to the neighbouring pixels (Floyd-Steinberg), so the tones survive as a dot pattern.
    /// Alpha is left alone. 16-bit images are dithered at 8 bits and widened again.
    ///
    /// # Errors
    ///
    /// Returns a message for `RG16` and for every non-greyscale format.
    pub fn apply_dithering(&mut self) -> Result<(), String> {
        if !self.is_greyscale_format() {
            return Err("Dithering can only be applied to greyscale images".to_string());
        }

        match (&mut self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::R8) => {
                let width = self.width;
                let height = self.height;
                let mut grid = Vec::with_capacity(height);

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

                for y in 0..height {
                    for x in 0..width {
                        let old_pixel = grid[y][x];
                        let new_pixel = if old_pixel > 127 { 255 } else { 0 };
                        let quant_error = old_pixel - new_pixel;

                        grid[y][x] = new_pixel;

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
                                    (grid[y + 1][x + 1] + quant_error / 16).clamp(0, 255);
                            }
                        }
                    }
                }

                let mut result = Vec::with_capacity(data.len());
                for y in 0..height {
                    for x in 0..width {
                        result.push(grid[y][x] as u8);
                    }
                }

                *data = result;
                Ok(())
            }
            (RawImageData::U8(data), RawImageFormat::RG8) => {
                let mut grey = Vec::with_capacity(data.len() / 2);
                let mut alpha = Vec::with_capacity(data.len() / 2);

                for i in (0..data.len()).step_by(2) {
                    if i + 1 < data.len() {
                        grey.push(data[i]);
                        alpha.push(data[i + 1]);
                    }
                }

                let mut temp_img = RawImage {
                    pixels: RawImageData::U8(grey),
                    width: self.width,
                    height: self.height,
                    data_format: RawImageFormat::R8,
                    tag: Vec::new(),
                };

                temp_img.apply_dithering()?;

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
                let mut grey_u8 = Vec::with_capacity(data.len());

                for &value in data.iter() {
                    grey_u8.push((value >> 8) as u8);
                }

                let mut temp_img = RawImage {
                    pixels: RawImageData::U8(grey_u8),
                    width: self.width,
                    height: self.height,
                    data_format: RawImageFormat::R8,
                    tag: Vec::new(),
                };

                temp_img.apply_dithering()?;

                if let RawImageData::U8(dithered_grey) = &temp_img.pixels {
                    let mut result = Vec::with_capacity(data.len());
                    for &value in dithered_grey.iter() {
                        result.push((value as u16) << 8 | (value as u16));
                    }
                    *data = result;
                }

                Ok(())
            }
            _ => Err("Unsupported format for dithering".to_string()),
        }
    }

    /// The size of the pixel data in bytes, uncompressed: `width * height` times the bytes per
    /// pixel of the format.
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

        (self.width * self.height * bits_per_pixel).div_ceil(8)
    }

    /// Scales the image down, keeping its aspect ratio, until its uncompressed size
    /// ([`RawImage::estimate_size_bytes`]) is at most `max_size_bytes`. Nearest-neighbour
    /// sampling, so the picture loses detail. Does nothing if it is small enough already.
    ///
    /// # Errors
    ///
    /// Returns a message when scaling is needed but the format is not an 8-bit one (`R8`, `RG8`,
    /// `RGB8`, `RGBA8`, `BGR8`, `BGRA8`).
    pub fn resize_to_fit_size(&mut self, max_size_bytes: usize) -> Result<(), String> {
        let current_size = self.estimate_size_bytes();
        if current_size <= max_size_bytes {
            return Ok(());
        }

        let scale_factor = (max_size_bytes as f64 / current_size as f64).sqrt();

        let new_width = (self.width as f64 * scale_factor).round() as usize;
        let new_height = (self.height as f64 * scale_factor).round() as usize;

        let new_width = new_width.max(1);
        let new_height = new_height.max(1);

        match (&self.pixels, self.data_format) {
            (RawImageData::U8(data), RawImageFormat::RGB8) => {
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

/// Encodes an image as an image XObject stream: optimizes it with `options`, splits the alpha
/// channel into a `/SMask`, and applies the filter that `options` choose. Without `options`
/// the samples are written unfiltered.
pub(crate) fn image_to_stream(
    im: RawImage,
    doc: &mut lopdf::Document,
    options: Option<&ImageOptimizationOptions>,
) -> lopdf::Stream {
    use lopdf::Object::*;

    let mut im = im;
    if let Some(opts) = options {
        let _ = im.optimize(opts);
    }

    let (rgb8, alpha) = split_rawimage_into_rgb_plus_alpha(im);
    let (bpc, cs) = rgb8.data_format.get_color_bits_and_space();
    // A greyscale picture that is only black and white (line art, or one that `dither_greyscale`
    // made so) is written with one bit per pixel: the same picture in an eighth of the data.
    let bits = pack_bilevel(&rgb8);
    let bpc = if bits.is_some() { ColorBits::Bit1 } else { bpc };
    let pixels: &[u8] = bits.as_deref().unwrap_or(&rgb8.pixels);
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

    let mut compressed_pixels = pixels.to_vec();
    if let Some(opts) = options {
        if let Some(filter) = get_compression_filter(opts, &rgb8) {
            // JPEG has no one-bit pixels, and Flate is lossless and smaller for black and white
            let filter = if bits.is_some() && filter == "DCTDecode" {
                "FlateDecode"
            } else {
                filter
            };
            match filter {
                #[cfg(feature = "jpeg")]
                "DCTDecode" => {
                    let quality = opts.quality.unwrap_or(0.85);
                    if let Some(jpeg_data) = jpeg_encode(&rgb8, quality) {
                        compressed_pixels = jpeg_data;
                        dict.set("Filter", Name(filter.into()));
                    }
                }
                "FlateDecode" => {
                    if let Some(flate_data) = flate_encode(pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name(filter.into()));
                    }
                }
                // Flate stands in for LZW and run-length: `lzw_encode` and `rle_encode` exist
                // but are not enabled.
                "LZWDecode" => {
                    if let Some(flate_data) = flate_encode(pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                }
                "RunLengthDecode" => {
                    if let Some(flate_data) = flate_encode(pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                }
                _ => {}
            }
        }
    }

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

        // The alpha channel is always lossless.
        let mut alpha_pixels = alpha.pixels.clone();
        let mut alpha_filter_applied = false;

        if let Some(opts) = options {
            let format = opts.format.unwrap_or_default();

            let alpha_format = match format {
                ImageCompression::Auto | ImageCompression::Jpeg | ImageCompression::Jpeg2000 => {
                    ImageCompression::Flate
                }
                _ => format,
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
                    if let Some(flate_data) = flate_encode(pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                }
                "RunLengthDecode" => {
                    if let Some(flate_data) = flate_encode(pixels) {
                        compressed_pixels = flate_data;
                        dict.set("Filter", Name("FlateDecode".into()));
                    }
                }
                _ => {}
            }
        }

        if !alpha_filter_applied {
            if let Some(flate_data) = flate_encode(&alpha_pixels) {
                alpha_pixels = flate_data;
                smask_dict.set("Filter", Name("FlateDecode".into()));
            }
        }

        // The data is already encoded, so the writer must not compress it again.
        let mut stream = lopdf::Stream::new(smask_dict, alpha_pixels);
        stream = stream.with_compression(false);

        dict.set("SMask", Reference(doc.add_object(stream)));
    }

    let mut s = lopdf::Stream::new(dict, compressed_pixels);
    s = s.with_compression(false);

    s
}

// The PDF filter to encode the main image with, None for no filter.
fn get_compression_filter(
    opts: &ImageOptimizationOptions,
    image: &RawImageU8,
) -> Option<&'static str> {
    let is_grayscale = matches!(image.data_format, RawImageFormat::R8 | RawImageFormat::RG8);
    let dithering_enabled = opts.dither_greyscale.unwrap_or_default();

    // JPEG would blur a dither pattern.
    if is_grayscale && dithering_enabled {
        return Some("LZWDecode");
    }

    match opts.format.unwrap_or_default() {
        ImageCompression::Auto => {
            if matches!(image.data_format, RawImageFormat::R8 | RawImageFormat::RG8) {
                Some("LZWDecode")
            } else {
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

#[cfg(feature = "jpeg")]
fn jpeg_encode(image: &RawImageU8, quality: f32) -> Option<Vec<u8>> {
    let quality = (quality * 100.0) as u8;

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

    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    if img.write_with_encoder(encoder).is_ok() {
        Some(jpeg_data)
    } else {
        None
    }
}

/// The pixels of a greyscale picture that is only black and white, one bit per pixel (1 is
/// white, as DeviceGray reads it) and each row padded to whole bytes, as /BitsPerComponent 1
/// lays them out. None if the picture has a grey pixel, or colour.
fn pack_bilevel(image: &RawImageU8) -> Option<Vec<u8>> {
    if image.data_format != RawImageFormat::R8
        || image.width == 0
        || image.pixels.len() != image.width * image.height
        || image.pixels.iter().any(|&p| p != 0 && p != 255)
    {
        return None;
    }
    let mut bits = Vec::with_capacity(image.width.div_ceil(8) * image.height);
    for row in image.pixels.chunks(image.width) {
        for eight in row.chunks(8) {
            let byte =
                eight.iter().enumerate().fold(
                    0u8,
                    |byte, (i, &p)| {
                        if p == 255 {
                            byte | 0x80 >> i
                        } else {
                            byte
                        }
                    },
                );
            bits.push(byte);
        }
    }
    Some(bits)
}

fn flate_encode(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Write;

    use flate2::{write::ZlibEncoder, Compression};

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).ok()?;
    encoder.finish().ok()
}

#[allow(dead_code)]
fn lzw_encode(data: &[u8]) -> Option<Vec<u8>> {
    use weezl::{encode::Encoder, BitOrder};
    // PDF uses MSB-first codes
    let mut encoder = Encoder::new(BitOrder::Msb, 8);
    encoder.encode(data).ok()
}

#[allow(dead_code)]
fn rle_encode(data: &[u8]) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        let mut run_length = 1;
        let current_byte = data[i];

        while i + run_length < data.len()
            && data[i + run_length] == current_byte
            && run_length < 128
        {
            run_length += 1;
        }

        if run_length > 1 {
            result.push((run_length - 1) as u8);
            result.push(current_byte);
            i += run_length;
        } else {
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

            result.push((257 - literal_count) as u8);
            for j in 0..literal_count {
                result.push(data[literal_start + j]);
            }
        }
    }

    result.push(128);

    Some(result)
}

// Splits the alpha channel off as its own image, for the `/SMask`. 16-bit samples become
// big-endian bytes, as PDF reads them.
fn split_rawimage_into_rgb_plus_alpha(im: RawImage) -> (RawImageU8, Option<RawImageU8>) {
    let has_alpha = im.data_format.has_alpha();
    let (orig, alpha) = if has_alpha {
        match im.data_format {
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

#[cfg(all(
    feature = "js-sys",
    target_arch = "wasm32",
    not(any(target_env = "p1", target_env = "p2"))
))]
mod browser_image {

    use js_sys::{Array, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        js_sys, window, Blob, BlobPropertyBag, CanvasRenderingContext2d, HtmlCanvasElement,
        ImageBitmap,
    };

    use super::{OutputImageFormat, RawImage, RawImageData, RawImageFormat};
    use crate::PdfWarnMsg;

    pub async fn decode_image_with_browser(
        bytes: &[u8],
        _warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<RawImage, String> {
        let window = window().ok_or("No window available")?;

        let array = Array::new();
        let uint8_array = Uint8Array::from(bytes);
        array.push(&uint8_array.buffer());

        let options = BlobPropertyBag::new();
        let blob = Blob::new_with_u8_array_sequence_and_options(&array, &options)
            .map_err(|e| format!("Failed to create Blob: {:?}", e))?;

        let promise = window
            .create_image_bitmap_with_blob(&blob)
            .map_err(|e| format!("Failed to create ImageBitmap: {:?}", e))?;

        let bitmap: ImageBitmap = JsFuture::from(promise)
            .await
            .map_err(|e| format!("Promise rejected: {:?}", e))?
            .dyn_into()
            .map_err(|_| "Failed to cast to ImageBitmap")?;

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

        Ok(RawImage {
            pixels: RawImageData::U8(data_vec),
            width,
            height,
            data_format: RawImageFormat::RGBA8,
            tag: Vec::new(),
        })
    }

    pub async fn encode_image_with_browser(
        image: &RawImage,
        format: OutputImageFormat,
    ) -> Result<Vec<u8>, String> {
        let mime_type = match format {
            OutputImageFormat::Jpeg => "image/jpeg",
            OutputImageFormat::Png => "image/png",
            OutputImageFormat::Webp => "image/webp",
            _ => return Err(format!("Format {:?} not supported by browser", format)),
        };

        let pixels = match &image.pixels {
            RawImageData::U8(data) => data,
            _ => return Err("Only U8 data is supported for browser encoding".to_string()),
        };

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

        let data_url = canvas
            .to_data_url_with_type(mime_type)
            .map_err(|_| format!("Failed to encode to {}", mime_type))?;

        let bytes = crate::Base64OrRaw::B64(data_url).decode_bytes()?;

        Ok(bytes)
    }
}

// -- ⚝  by Dave -- in NeoVim ⚝  --
