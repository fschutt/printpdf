//! Images that disagree with themselves, files that ask for too much, and pages that nest too
//! deeply must give an error, a warning or a degraded result. They must not panic, hang or
//! allocate from an unchecked size (on `wasm32` a panic traps the whole module).

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::time::Duration;

use printpdf::*;

/// Runs `f` on its own thread and waits at most `limit`. A panic of `f` is raised again here; a
/// timeout fails the test, so a hang cannot hold the build lock.
fn bounded_for<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limit) {
        Ok(value) => {
            let _ = worker.join();
            value
        }
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("no result after {limit:?} (hang)"),
        Err(mpsc::RecvTimeoutError::Disconnected) => match worker.join() {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(()) => panic!("the worker ended without a result"),
        },
    }
}

fn bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    bounded_for(Duration::from_secs(5), f)
}

const ALL_FORMATS: [RawImageFormat; 12] = [
    RawImageFormat::R8,
    RawImageFormat::RG8,
    RawImageFormat::RGB8,
    RawImageFormat::RGBA8,
    RawImageFormat::R16,
    RawImageFormat::RG16,
    RawImageFormat::RGB16,
    RawImageFormat::RGBA16,
    RawImageFormat::BGR8,
    RawImageFormat::BGRA8,
    RawImageFormat::RGBF32,
    RawImageFormat::RGBAF32,
];

/// An image of `format` that claims `width` x `height` pixels and holds `samples` samples (not
/// pixels), of the sample type the format uses.
fn image(format: RawImageFormat, width: usize, height: usize, samples: usize) -> RawImage {
    use RawImageFormat::*;
    let pixels = match format {
        R8 | RG8 | RGB8 | RGBA8 | BGR8 | BGRA8 => {
            RawImageData::U8((0..samples).map(|i| (i * 37 % 256) as u8).collect())
        }
        R16 | RG16 | RGB16 | RGBA16 => {
            RawImageData::U16((0..samples).map(|i| (i * 4099 % 65536) as u16).collect())
        }
        RGBF32 | RGBAF32 => RawImageData::F32((0..samples).map(|i| (i % 7) as f32 / 7.0).collect()),
    };
    RawImage {
        pixels,
        width,
        height,
        data_format: format,
        tag: Vec::new(),
    }
}

fn page_showing(image: &RawImage) -> (PdfDocument, PdfPage) {
    let mut doc = PdfDocument::new("no panic");
    let id = doc.add_image(image);
    let ops = vec![Op::UseXobject {
        id,
        transform: XObjectTransform::default(),
    }];
    (doc, PdfPage::new(Mm(50.0), Mm(50.0), ops))
}

/// Saves a one-page document that shows `image`.
fn save(image: &RawImage, options: &PdfSaveOptions) -> Vec<u8> {
    let (mut doc, page) = page_showing(image);
    doc.with_pages(vec![page]).save(options, &mut Vec::new())
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

/// Sizes that are zero, small, or so large that `width * height` overflows `usize`. Sizes in
/// between would make a decoder or an unchecked allocation try for gigabytes, which is not what
/// these tests are about.
const SIZES: [usize; 9] = [
    0,
    1,
    2,
    3,
    5,
    8,
    usize::MAX,
    usize::MAX / 2,
    usize::MAX / 3,
];

#[cfg(feature = "images")]
mod with_images {
    use super::*;

    fn grey(width: usize, height: usize, samples: usize) -> RawImage {
        image(RawImageFormat::R8, width, height, samples)
    }

    fn options_without_images() -> PdfSaveOptions {
        PdfSaveOptions {
            image_optimization: None,
            ..Default::default()
        }
    }

    /// Option sets that reach every step of `image_to_stream`.
    fn option_sets() -> Vec<Option<ImageOptimizationOptions>> {
        vec![
            Some(ImageOptimizationOptions::default()),
            None,
            Some(ImageOptimizationOptions {
                format: Some(ImageCompression::Flate),
                max_image_size: None,
                ..Default::default()
            }),
            Some(ImageOptimizationOptions {
                format: Some(ImageCompression::Jpeg),
                convert_to_greyscale: Some(true),
                max_image_size: Some("1b".to_string()),
                ..Default::default()
            }),
            Some(ImageOptimizationOptions {
                dither_greyscale: Some(true),
                max_image_size: Some("1b".to_string()),
                quality: Some(f32::NAN),
                ..Default::default()
            }),
            Some(ImageOptimizationOptions {
                format: Some(ImageCompression::Lzw),
                dither_greyscale: Some(true),
                auto_optimize: Some(false),
                quality: Some(f32::INFINITY),
                max_image_size: Some("0b".to_string()),
                ..Default::default()
            }),
        ]
    }

    fn save_with(image: &RawImage, image_optimization: Option<ImageOptimizationOptions>) -> Vec<u8> {
        save(
            image,
            &PdfSaveOptions {
                image_optimization,
                ..Default::default()
            },
        )
    }

    // --- saving documents: image_to_stream ---

    #[test]
    fn rgb8_with_extra_bytes_is_saved() {
        // 2 x 2 RGB is 12 bytes; the JPEG encoder panics on a buffer of another length.
        let bytes = bounded(|| save(&image(RawImageFormat::RGB8, 2, 2, 13), &PdfSaveOptions::default()));
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn grey8_with_extra_bytes_is_saved() {
        let bytes = bounded(|| save(&grey(2, 2, 9), &PdfSaveOptions::default()));
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn rgb8_with_missing_bytes_is_saved() {
        let bytes = bounded(|| save(&image(RawImageFormat::RGB8, 4, 4, 5), &PdfSaveOptions::default()));
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn bgr8_with_extra_bytes_is_saved() {
        let bytes = bounded(|| save(&image(RawImageFormat::BGR8, 2, 2, 14), &PdfSaveOptions::default()));
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn grey_alpha_with_an_odd_sample_count_is_saved() {
        for options in [PdfSaveOptions::default(), options_without_images()] {
            let o = options.clone();
            bounded(move || save(&image(RawImageFormat::RG8, 1, 2, 5), &o));
            bounded(move || save(&image(RawImageFormat::RG16, 1, 2, 5), &options));
        }
    }

    #[test]
    fn rgba_with_a_partial_pixel_is_saved() {
        for options in [PdfSaveOptions::default(), options_without_images()] {
            let o = options.clone();
            bounded(move || save(&image(RawImageFormat::RGBA8, 1, 2, 7), &o));
            bounded(move || save(&image(RawImageFormat::BGRA8, 1, 2, 7), &options));
        }
    }

    #[test]
    fn a_huge_size_with_a_tiny_buffer_is_saved() {
        for size in [usize::MAX, usize::MAX / 2 + 1, usize::MAX / 3] {
            for options in [PdfSaveOptions::default(), options_without_images()] {
                let o = options.clone();
                bounded(move || save(&grey(size, 2, 3), &o));
                bounded(move || save(&grey(2, size, 3), &options));
            }
        }
    }

    #[test]
    fn zero_sized_images_are_saved() {
        for (w, h, n) in [(0, 0, 0), (0, 5, 0), (5, 0, 0), (0, 0, 4), (3, 0, 9)] {
            for format in ALL_FORMATS {
                bounded(move || save(&image(format, w, h, n), &PdfSaveOptions::default()));
            }
        }
    }

    #[test]
    fn every_format_with_odd_buffers_is_saved_with_every_option_set() {
        let mut failures = Vec::new();
        for format in ALL_FORMATS {
            for (w, h) in [(1, 1), (2, 3), (3, 1)] {
                for samples in [0, 1, 2, 3, 5, 7, 13, 50] {
                    for options in option_sets() {
                        let im = image(format, w, h, samples);
                        let o = options.clone();
                        if catch_unwind(AssertUnwindSafe(|| save_with(&im, o))).is_err() {
                            failures.push(format!("{format:?} {w}x{h} {samples} samples {options:?}"));
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "panics on: {failures:#?}");
    }

    // --- encode_to_bytes ---

    const OUTPUT_FORMATS: [OutputImageFormat; 9] = [
        OutputImageFormat::Png,
        OutputImageFormat::Jpeg,
        OutputImageFormat::Webp,
        OutputImageFormat::Avif,
        OutputImageFormat::Gif,
        OutputImageFormat::Bmp,
        OutputImageFormat::Tiff,
        OutputImageFormat::Tga,
        OutputImageFormat::Pnm,
    ];

    #[test]
    fn a_buffer_of_the_wrong_length_is_an_encode_error() {
        let mut panicked = Vec::new();
        let mut accepted = Vec::new();
        for format in [
            RawImageFormat::R8,
            RawImageFormat::RG8,
            RawImageFormat::RGB8,
            RawImageFormat::RGBA8,
        ] {
            let channels = match format {
                RawImageFormat::R8 => 1,
                RawImageFormat::RG8 => 2,
                RawImageFormat::RGB8 => 3,
                _ => 4,
            };
            for samples in [0, 1, 4 * channels - 1, 4 * channels + 1, 5 * channels] {
                for target in OUTPUT_FORMATS {
                    let im = image(format, 2, 2, samples);
                    match catch_unwind(AssertUnwindSafe(|| im.encode_to_bytes(&[target]))) {
                        Err(_) => panicked.push(format!("{format:?} {samples} samples to {target:?}")),
                        Ok(Ok(_)) => accepted.push(format!("{format:?} {samples} samples to {target:?}")),
                        Ok(Err(_)) => {}
                    }
                }
            }
        }
        assert!(panicked.is_empty(), "panics on: {panicked:#?}");
        assert!(accepted.is_empty(), "encoded a buffer of the wrong length: {accepted:#?}");
    }

    #[test]
    fn zero_sized_and_huge_images_are_encode_errors() {
        let mut panicked = Vec::new();
        for format in ALL_FORMATS {
            for (w, h) in [(0, 0), (0, 3), (3, 0), (usize::MAX, 2), (2, usize::MAX / 2), (usize::MAX / 5, 1)] {
                for target in OUTPUT_FORMATS {
                    let im = image(format, w, h, 3);
                    match catch_unwind(AssertUnwindSafe(|| im.encode_to_bytes(&[target]))) {
                        Err(_) => panicked.push(format!("{format:?} {w}x{h} to {target:?}")),
                        Ok(result) => assert!(result.is_err(), "{format:?} {w}x{h} to {target:?}"),
                    }
                }
            }
        }
        assert!(panicked.is_empty(), "panics on: {panicked:#?}");
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "bmp"))]
    #[test]
    fn a_valid_image_still_encodes() {
        let im = image(RawImageFormat::RGB8, 2, 2, 12);
        for target in [OutputImageFormat::Png, OutputImageFormat::Bmp, OutputImageFormat::Jpeg] {
            let (bytes, used) = im.encode_to_bytes(&[target]).expect("a consistent image encodes");
            assert_eq!(used, target);
            assert!(!bytes.is_empty());
        }
    }

    #[test]
    fn to_svg_skips_an_image_with_a_buffer_of_the_wrong_length() {
        let svg = bounded(|| {
            let (doc, page) = page_showing(&image(RawImageFormat::RGB8, 2, 2, 13));
            page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut Vec::new())
        });
        assert!(svg.starts_with("<svg"), "{svg}");
        assert!(!svg.contains("<image"), "{svg}");
    }

    // --- the pixel operations ---

    #[test]
    fn dithering_a_buffer_of_the_wrong_length_is_an_error() {
        let mut im = grey(3, 3, 4);
        let result = bounded(move || im.apply_dithering());
        assert!(result.is_err());
    }

    #[test]
    fn dithering_a_huge_size_is_an_error() {
        for (w, h) in [(usize::MAX / 2, 3), (3, usize::MAX / 2), (usize::MAX, usize::MAX)] {
            let mut im = grey(w, h, 3);
            let result = bounded(move || im.apply_dithering());
            assert!(result.is_err(), "{w}x{h}");
        }
    }

    #[test]
    fn dithering_a_valid_image_is_unchanged() {
        // a 3 x 2 ramp: 0 16 32 / 64 128 255
        let mut im = RawImage {
            pixels: RawImageData::U8(vec![0, 16, 32, 64, 128, 255]),
            width: 3,
            height: 2,
            data_format: RawImageFormat::R8,
            tag: Vec::new(),
        };
        im.apply_dithering().unwrap();
        match &im.pixels {
            RawImageData::U8(p) => assert_eq!(p, &vec![0, 0, 0, 0, 255, 255]),
            _ => panic!("expected 8-bit pixels"),
        }
    }

    #[test]
    fn estimate_size_bytes_does_not_overflow() {
        for (w, h) in [(usize::MAX, 2), (usize::MAX / 4, usize::MAX / 4), (usize::MAX / 2, usize::MAX / 2)] {
            for format in ALL_FORMATS {
                let size = bounded(move || image(format, w, h, 0).estimate_size_bytes());
                assert!(size >= 1 << 30, "{format:?} {w}x{h}: {size}");
            }
        }
        assert_eq!(image(RawImageFormat::RGB8, 10, 10, 0).estimate_size_bytes(), 300);
    }

    #[test]
    fn resizing_a_hostile_image_does_not_panic_or_allocate() {
        for format in ALL_FORMATS {
            for (w, h, samples) in [
                (usize::MAX, 2, 3),
                (usize::MAX / 4, usize::MAX / 4, 3),
                (usize::MAX / 4, 1, 3),
                (1000, 1000, 10),
                (usize::MAX / 2, usize::MAX / 2, 0),
            ] {
                let mut im = image(format, w, h, samples);
                let result = bounded(move || {
                    let r = im.resize_to_fit_size(100);
                    (r, im.pixels.is_empty(), im.width, im.height)
                });
                // either refused, or untouched, or scaled to something small
                if result.0.is_ok() && !result.1 {
                    assert!(result.2.saturating_mul(result.3) <= 100, "{format:?} {w}x{h}");
                }
            }
        }
    }

    #[test]
    fn resizing_a_valid_image_is_unchanged() {
        // 4 x 4 grey, 4 bytes allowed: scaled to 2 x 2 by taking every second pixel
        let mut im = RawImage {
            pixels: RawImageData::U8((0..16).collect()),
            width: 4,
            height: 4,
            data_format: RawImageFormat::R8,
            tag: Vec::new(),
        };
        im.resize_to_fit_size(4).unwrap();
        assert_eq!((im.width, im.height), (2, 2));
        match &im.pixels {
            RawImageData::U8(p) => assert_eq!(p, &vec![0, 2, 8, 10]),
            _ => panic!("expected 8-bit pixels"),
        }
    }

    #[test]
    fn the_pixel_operations_accept_every_buffer() {
        let mut failures = Vec::new();
        for format in ALL_FORMATS {
            for (w, h) in [(0, 0), (1, 1), (2, 2), (3, 1), (1, 3)] {
                for samples in [0, 1, 2, 3, 5, 7, 13, 50] {
                    let mut probe = |what: &str, f: &dyn Fn(RawImage)| {
                        let im = image(format, w, h, samples);
                        if catch_unwind(AssertUnwindSafe(|| f(im))).is_err() {
                            failures.push(format!("{what}: {format:?} {w}x{h} {samples} samples"));
                        }
                    };
                    probe("is_fully_opaque", &|im| {
                        im.is_fully_opaque();
                    });
                    probe("is_actually_greyscale", &|im| {
                        im.is_actually_greyscale();
                    });
                    probe("remove_alpha_channel", &|mut im| {
                        let _ = im.remove_alpha_channel();
                    });
                    probe("convert_to_greyscale", &|mut im| {
                        let _ = im.convert_to_greyscale();
                    });
                    probe("apply_dithering", &|mut im| {
                        let _ = im.apply_dithering();
                    });
                    probe("resize_to_fit_size", &|mut im| {
                        let _ = im.resize_to_fit_size(3);
                    });
                    probe("estimate_size_bytes", &|im| {
                        im.estimate_size_bytes();
                    });
                    for options in option_sets().into_iter().flatten() {
                        probe("optimize", &|mut im| {
                            let _ = im.optimize(&options);
                        });
                    }
                }
            }
        }
        assert!(failures.is_empty(), "panics: {failures:#?}");
    }

    // --- valid images keep their results ---

    fn rgba8(samples: &[u8]) -> RawImage {
        RawImage {
            pixels: RawImageData::U8(samples.to_vec()),
            width: samples.len() / 4,
            height: 1,
            data_format: RawImageFormat::RGBA8,
            tag: Vec::new(),
        }
    }

    fn u8_pixels(image: &RawImage) -> &[u8] {
        match &image.pixels {
            RawImageData::U8(p) => p,
            other => panic!("expected 8-bit pixels, got {other:?}"),
        }
    }

    fn u16_pixels(image: &RawImage) -> &[u16] {
        match &image.pixels {
            RawImageData::U16(p) => p,
            other => panic!("expected 16-bit pixels, got {other:?}"),
        }
    }

    #[test]
    fn alpha_and_grey_detection_on_valid_images() {
        assert!(rgba8(&[1, 2, 3, 255, 4, 5, 6, 255]).is_fully_opaque());
        assert!(!rgba8(&[1, 2, 3, 255, 4, 5, 6, 254]).is_fully_opaque());
        assert!(!image(RawImageFormat::RGB8, 1, 1, 3).is_fully_opaque());

        let mut im = rgba8(&[1, 2, 3, 255, 4, 5, 6, 255]);
        im.remove_alpha_channel().unwrap();
        assert_eq!(im.data_format, RawImageFormat::RGB8);
        assert_eq!(u8_pixels(&im), &[1, 2, 3, 4, 5, 6]);

        let mut im = RawImage {
            pixels: RawImageData::U16(vec![1, 2, 3, 65535, 4, 5, 6, 65535]),
            width: 2,
            height: 1,
            data_format: RawImageFormat::RGBA16,
            tag: Vec::new(),
        };
        assert!(im.is_fully_opaque());
        im.remove_alpha_channel().unwrap();
        assert_eq!(u16_pixels(&im), &[1, 2, 3, 4, 5, 6]);

        let rgb = |samples: &[u8]| RawImage {
            pixels: RawImageData::U8(samples.to_vec()),
            width: samples.len() / 3,
            height: 1,
            data_format: RawImageFormat::RGB8,
            tag: Vec::new(),
        };
        assert!(rgb(&[10, 12, 9, 200, 200, 203]).is_actually_greyscale());
        assert!(!rgb(&[10, 12, 9, 200, 200, 204]).is_actually_greyscale());
        assert!(rgba8(&[10, 12, 9, 7, 200, 200, 203, 7]).is_actually_greyscale());
        assert!(!rgba8(&[10, 12, 9, 7, 200, 200, 204, 7]).is_actually_greyscale());
    }

    #[test]
    fn grey_conversion_on_valid_images() {
        let convert = |format, samples: &[u8]| {
            let mut im = RawImage {
                pixels: RawImageData::U8(samples.to_vec()),
                width: samples.len() / image_channels(format),
                height: 1,
                data_format: format,
                tag: Vec::new(),
            };
            im.convert_to_greyscale().unwrap();
            (im.data_format, u8_pixels(&im).to_vec())
        };
        fn image_channels(format: RawImageFormat) -> usize {
            match format {
                RawImageFormat::RGB8 | RawImageFormat::BGR8 => 3,
                _ => 4,
            }
        }
        assert_eq!(
            convert(RawImageFormat::RGB8, &[255, 0, 0, 0, 255, 0, 0, 0, 255]),
            (RawImageFormat::R8, vec![76, 149, 29])
        );
        assert_eq!(
            convert(RawImageFormat::BGR8, &[0, 0, 255, 0, 255, 0]),
            (RawImageFormat::R8, vec![76, 149])
        );
        assert_eq!(
            convert(RawImageFormat::RGBA8, &[255, 0, 0, 7, 0, 0, 255, 9]),
            (RawImageFormat::RG8, vec![76, 7, 29, 9])
        );
        assert_eq!(
            convert(RawImageFormat::BGRA8, &[0, 0, 255, 5, 255, 0, 0, 6]),
            (RawImageFormat::RG8, vec![76, 5, 29, 6])
        );

        let mut im = RawImage {
            pixels: RawImageData::U16(vec![65535, 0, 0, 7, 0, 0, 65535, 9]),
            width: 2,
            height: 1,
            data_format: RawImageFormat::RGBA16,
            tag: Vec::new(),
        };
        im.convert_to_greyscale().unwrap();
        assert_eq!(im.data_format, RawImageFormat::RG16);
        assert_eq!(u16_pixels(&im), &[19594, 7, 7470, 9]);

        let mut im = RawImage {
            pixels: RawImageData::U16(vec![0, 65535, 0, 65535, 0, 0]),
            width: 2,
            height: 1,
            data_format: RawImageFormat::RGB16,
            tag: Vec::new(),
        };
        im.convert_to_greyscale().unwrap();
        assert_eq!(im.data_format, RawImageFormat::R16);
        assert_eq!(u16_pixels(&im), &[38469, 19594]);
    }

    #[test]
    fn dithering_grey_with_alpha_and_16_bit_on_valid_images() {
        let mut im = RawImage {
            pixels: RawImageData::U8(vec![0, 10, 255, 20]),
            width: 2,
            height: 1,
            data_format: RawImageFormat::RG8,
            tag: Vec::new(),
        };
        im.apply_dithering().unwrap();
        assert_eq!(u8_pixels(&im), &[0, 10, 255, 20]);

        let mut im = RawImage {
            pixels: RawImageData::U16(vec![0, 0xFFFF]),
            width: 2,
            height: 1,
            data_format: RawImageFormat::R16,
            tag: Vec::new(),
        };
        im.apply_dithering().unwrap();
        assert_eq!(u16_pixels(&im), &[0, 0xFFFF]);
    }

    #[test]
    fn resizing_every_8_bit_format_on_valid_images() {
        for (format, channels) in [
            (RawImageFormat::R8, 1),
            (RawImageFormat::RG8, 2),
            (RawImageFormat::RGB8, 3),
            (RawImageFormat::BGR8, 3),
            (RawImageFormat::RGBA8, 4),
            (RawImageFormat::BGRA8, 4),
        ] {
            // 4 x 4 pixels, every sample different; a quarter of the size keeps pixels 0, 2, 8, 10
            let mut im = image(format, 4, 4, 16 * channels);
            let original = u8_pixels(&im).to_vec();
            im.resize_to_fit_size(4 * channels).unwrap();
            assert_eq!((im.width, im.height), (2, 2), "{format:?}");
            let expected: Vec<u8> = [0usize, 2, 8, 10]
                .iter()
                .flat_map(|p| original[p * channels..(p + 1) * channels].to_vec())
                .collect();
            assert_eq!(u8_pixels(&im), &expected[..], "{format:?}");
        }
    }

    #[test]
    fn alpha_is_split_off_into_a_soft_mask_on_valid_images() {
        fn image_streams(pdf: &[u8]) -> Vec<(lopdf::Dictionary, Vec<u8>)> {
            let doc = lopdf::Document::load_mem(pdf).unwrap();
            let streams: Vec<_> = doc
                .objects
                .values()
                .filter_map(|object| match object {
                    lopdf::Object::Stream(s)
                        if s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok()
                            == Some(b"Image") =>
                    {
                        let data = if s.dict.has(b"Filter") {
                            s.decompressed_content().unwrap()
                        } else {
                            s.content.clone()
                        };
                        Some((s.dict.clone(), data))
                    }
                    _ => None,
                })
                .collect();
            streams
        }

        // the color samples of the image that has a soft mask, and the samples of the mask
        fn color_and_mask(pdf: &[u8]) -> (Vec<u8>, Vec<u8>) {
            let streams = image_streams(pdf);
            assert_eq!(streams.len(), 2);
            let (with_mask, without_mask): (Vec<_>, Vec<_>) =
                streams.into_iter().partition(|(dict, _)| dict.has(b"SMask"));
            assert_eq!((with_mask.len(), without_mask.len()), (1, 1));
            (with_mask[0].1.clone(), without_mask[0].1.clone())
        }

        let rgba = rgba8(&[10, 20, 30, 255, 40, 50, 60, 128]);
        let (color, mask) = color_and_mask(&save(&rgba, &options_without_images()));
        assert_eq!(color, vec![10, 20, 30, 40, 50, 60]);
        assert_eq!(mask, vec![255, 128]);

        // samples 0, 37, 74, 111: the pixels are (0, 37) and (74, 111)
        let grey_alpha = image(RawImageFormat::RG8, 2, 1, 4);
        let (color, mask) = color_and_mask(&save(&grey_alpha, &options_without_images()));
        assert_eq!(color, vec![0, 74]);
        assert_eq!(mask, vec![37, 111]);
    }

    #[test]
    fn parse_size_string_accepts_any_text() {
        for text in [
            "", " ", "b", "kb", "1", "1.5.5mb", "...", "9999999999999999999999gb", "-1mb", "1e9b",
            "é", "1é", "0.0000001b", "NaN", "inf", "١٢٣kb",
        ] {
            let _ = parse_size_string(text);
        }
        assert_eq!(parse_size_string("300kb"), Ok(300 * 1024));
    }

    // --- random images through every path ---

    #[test]
    fn random_images_are_saved_encoded_and_rendered() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let sets = option_sets();
        let mut failures = Vec::new();
        for round in 0..300 {
            let format = rng.pick(&ALL_FORMATS);
            let (w, h) = (rng.pick(&SIZES), rng.pick(&SIZES));
            let samples = rng.below(40);
            let mut im = image(format, w, h, samples);
            // sometimes with the wrong sample type for the format
            if rng.below(8) == 0 {
                im.pixels = match rng.below(3) {
                    0 => RawImageData::U8(vec![1; samples]),
                    1 => RawImageData::U16(vec![1; samples]),
                    _ => RawImageData::F32(vec![0.5; samples]),
                };
            }
            let options = sets[rng.below(sets.len())].clone();
            let result = catch_unwind(AssertUnwindSafe(|| {
                let (mut doc, page) = page_showing(&im);
                doc.with_pages(vec![page.clone()]);
                let options = PdfSaveOptions {
                    image_optimization: options.clone(),
                    ..Default::default()
                };
                let _ = doc.save(&options, &mut Vec::new());
                let _ = page.to_svg(&doc.resources, &PdfToSvgOptions::web(), &mut Vec::new());
                let _ = im.encode_to_bytes(&[OutputImageFormat::Png, OutputImageFormat::Jpeg]);
            }));
            if result.is_err() {
                failures.push(format!("round {round}: {format:?} {w}x{h} {samples} samples"));
            }
        }
        assert!(failures.is_empty(), "panics: {failures:#?}");
    }

    // --- decoding files ---

    /// A PNG file whose header says `width` x `height` and that has no pixel data to speak of.
    #[cfg(feature = "png")]
    fn png_with_header(width: u32, height: u32, bit_depth: u8, color_type: u8) -> Vec<u8> {
        fn crc32(data: &[u8]) -> u32 {
            let mut crc = 0xFFFF_FFFFu32;
            for &byte in data {
                crc ^= u32::from(byte);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 { 0xEDB8_8320 ^ (crc >> 1) } else { crc >> 1 };
                }
            }
            !crc
        }
        fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut out = (data.len() as u32).to_be_bytes().to_vec();
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc32(&body).to_be_bytes());
            out
        }
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[bit_depth, color_type, 0, 0, 0]);
        png.extend(chunk(b"IHDR", &ihdr));
        png.extend(chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]));
        png.extend(chunk(b"IEND", &[]));
        png
    }

    #[cfg(feature = "png")]
    #[test]
    fn a_header_that_asks_for_gigabytes_is_refused() {
        for (w, h, depth, color) in [
            (100_000, 100_000, 8, 2),
            (65_535, 65_535, 16, 6),
            (u32::MAX, u32::MAX, 16, 6),
            (i32::MAX as u32, 1, 8, 2),
            (1, i32::MAX as u32, 8, 0),
            (0, 0, 8, 2),
        ] {
            let bytes = png_with_header(w, h, depth, color);
            let result = bounded(move || RawImage::decode_from_bytes(&bytes, &mut Vec::new()));
            assert!(result.is_err(), "{w}x{h}");
        }
    }

    #[cfg(feature = "png")]
    #[test]
    fn a_picture_wider_than_the_limit_is_refused() {
        // 4 million pixels in one row is only 4 MB of grey, but no real picture is that wide
        let strip = grey(4_000_000, 1, 4_000_000);
        let (png, _) = strip.encode_to_bytes(&[OutputImageFormat::Png]).unwrap();
        let result = bounded(move || RawImage::decode_from_bytes(&png, &mut Vec::new()));
        assert!(result.is_err());
    }

    #[cfg(feature = "png")]
    #[test]
    fn an_ordinary_picture_still_decodes() {
        let (png, _) = image(RawImageFormat::RGB8, 3, 2, 18)
            .encode_to_bytes(&[OutputImageFormat::Png])
            .unwrap();
        let decoded = RawImage::decode_from_bytes(&png, &mut Vec::new()).unwrap();
        assert_eq!((decoded.width, decoded.height), (3, 2));
    }

    #[test]
    fn damaged_files_are_decode_errors_not_panics() {
        let sample = image(RawImageFormat::RGBA8, 5, 4, 80);
        let mut files = Vec::new();
        for target in [
            OutputImageFormat::Png,
            OutputImageFormat::Jpeg,
            OutputImageFormat::Gif,
            OutputImageFormat::Bmp,
            OutputImageFormat::Tiff,
            OutputImageFormat::Tga,
            OutputImageFormat::Pnm,
            OutputImageFormat::Webp,
        ] {
            if let Ok((bytes, _)) = sample.encode_to_bytes(&[target]) {
                files.push((target, bytes));
            }
        }
        let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
        let mut failures = Vec::new();
        for (target, bytes) in &files {
            // every truncation of the first 64 bytes, then flipped bytes
            let mut variants: Vec<Vec<u8>> = (0..bytes.len().min(64)).map(|n| bytes[..n].to_vec()).collect();
            for _ in 0..30 {
                let mut copy = bytes.clone();
                for _ in 0..=rng.below(3) {
                    let at = rng.below(copy.len());
                    copy[at] = rng.next() as u8;
                }
                variants.push(copy);
            }
            for (i, variant) in variants.into_iter().enumerate() {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let _ = RawImage::decode_from_bytes(&variant, &mut Vec::new());
                }));
                if outcome.is_err() {
                    failures.push(format!("{target:?} variant {i}: {variant:?}"));
                }
            }
        }
        assert!(!files.is_empty());
        assert!(failures.is_empty(), "panics: {failures:#?}");
    }

    #[test]
    fn bytes_that_are_no_image_are_decode_errors() {
        let inputs: [&[u8]; 10] = [
            b"",
            b"x",
            b"\x89PNG",
            b"\x89PNG\r\n\x1a\n",
            b"\xFF\xD8",
            b"\xFF\xD8\xFF",
            b"GIF89a",
            b"BM",
            b"RIFF\0\0\0\0WEBP",
            b"II*\0",
        ];
        for bytes in inputs {
            assert!(RawImage::decode_from_bytes(bytes, &mut Vec::new()).is_err());
        }
    }

    // --- rendering a page: the graphics state stack ---

    fn svg_of(ops: Vec<Op>) -> (String, Vec<PdfWarnMsg>) {
        bounded(move || {
            let doc = PdfDocument::new("state");
            let page = PdfPage::new(Mm(50.0), Mm(50.0), ops);
            let mut warnings = Vec::new();
            let svg = page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut warnings);
            (svg, warnings)
        })
    }

    #[test]
    fn the_graphics_state_stack_has_a_depth_limit() {
        let n = 2000;
        let mut ops = vec![Op::SaveGraphicsState; n];
        ops.extend(vec![Op::RestoreGraphicsState; n]);
        let (svg, warnings) = svg_of(ops);
        let opened = svg.matches("<g>").count();
        assert!(opened <= 256, "{opened} nested groups");
        assert_eq!(opened, svg.matches("</g>").count(), "groups are balanced");
        assert!(
            warnings.iter().any(|w| w.msg.contains("nest")),
            "a warning says the nesting is cut off: {warnings:?}"
        );
    }

    #[test]
    fn after_the_depth_limit_the_state_still_unwinds() {
        let rgb = |r, g, b| Color::Rgb(Rgb {
            r,
            g,
            b,
            icc_profile: None,
        });
        let line = Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(1.0), Mm(1.0)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(9.0), Mm(9.0)),
                    bezier: false,
                },
            ],
            is_closed: false,
        };
        let mut ops = vec![Op::SetOutlineColor { col: rgb(1.0, 0.0, 0.0) }];
        ops.extend(vec![Op::SaveGraphicsState; 300]);
        ops.push(Op::SetOutlineColor { col: rgb(0.0, 1.0, 0.0) });
        ops.extend(vec![Op::RestoreGraphicsState; 300]);
        ops.push(Op::DrawLine { line });
        let (svg, _) = svg_of(ops);
        assert!(svg.contains("stroke=\"rgb(255, 0, 0)\""), "{svg}");
        assert!(!svg.contains("rgb(0, 255, 0)"), "{svg}");
    }

    #[test]
    fn shallow_graphics_states_are_all_kept() {
        let mut ops = vec![Op::SaveGraphicsState; 20];
        ops.extend(vec![Op::RestoreGraphicsState; 20]);
        let (svg, warnings) = svg_of(ops);
        assert_eq!(svg.matches("<g>").count(), 20);
        assert_eq!(svg.matches("</g>").count(), 20);
        assert!(warnings.iter().all(|w| !w.msg.contains("nest")));
    }

    #[test]
    fn restoring_more_often_than_saving_is_harmless() {
        let (svg, _) = svg_of(vec![Op::RestoreGraphicsState; 5]);
        assert!(svg.starts_with("<svg"));
        assert!(!svg.contains("</g>"));
        let mut ops = vec![Op::SaveGraphicsState; 3];
        ops.extend(vec![Op::RestoreGraphicsState; 10]);
        let (svg, _) = svg_of(ops);
        assert_eq!(svg.matches("<g>").count(), svg.matches("</g>").count());
    }

    #[test]
    fn an_image_drawn_many_times_is_encoded_once() {
        let uses = 300;
        let (drawing, one_encoding, svg_len) = bounded_for(Duration::from_secs(120), move || {
            // a gradient: it takes a while to encode, and the file is small
            let side = 1500;
            let pixels: Vec<u8> = (0..side * side)
                .map(|i| ((i % side + i / side) % 256) as u8)
                .collect();
            let im = RawImage {
                pixels: RawImageData::U8(pixels),
                width: side,
                height: side,
                data_format: RawImageFormat::R8,
                tag: Vec::new(),
            };
            let started = std::time::Instant::now();
            im.encode_to_bytes(&[OutputImageFormat::Png]).unwrap();
            let one_encoding = started.elapsed();

            let mut doc = PdfDocument::new("many uses");
            let id = doc.add_image(&im);
            let ops = vec![
                Op::UseXobject {
                    id,
                    transform: XObjectTransform::default(),
                };
                uses
            ];
            let page = PdfPage::new(Mm(50.0), Mm(50.0), ops);
            let started = std::time::Instant::now();
            let svg = page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut Vec::new());
            (started.elapsed(), one_encoding, svg.len())
        });
        assert!(svg_len > 0);
        assert!(
            drawing < one_encoding * (uses as u32 / 4),
            "{uses} uses took {drawing:?}, one encoding takes {one_encoding:?}"
        );
    }

    #[test]
    fn many_marked_content_sequences_and_saves_do_not_multiply() {
        let n = 1500;
        let mut ops = vec![
            Op::BeginMarkedContent {
                tag: "Span".to_string()
            };
            n
        ];
        ops.extend(vec![Op::SaveGraphicsState; n]);
        ops.extend(vec![Op::RestoreGraphicsState; n]);
        let (svg, _) = svg_of(ops);
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn many_fonts_and_saves_do_not_multiply() {
        // every `q` copies the graphics state, so it must not hold a table that grows with the page
        let n = 20_000;
        let mut ops: Vec<Op> = (0..n)
            .map(|i| Op::SetFont {
                font: PdfFontHandle::External(FontId(format!("F{i}"))),
                size: Pt(10.0),
            })
            .collect();
        for _ in 0..n {
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::RestoreGraphicsState);
        }
        let (svg, _) = svg_of(ops);
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn a_font_size_survives_a_save_and_restore() {
        let font = FontId("F1".to_string());
        let ops = vec![
            Op::SetFont {
                font: PdfFontHandle::External(font.clone()),
                size: Pt(20.0),
            },
            Op::SaveGraphicsState,
            Op::SetFont {
                font: PdfFontHandle::External(font.clone()),
                size: Pt(30.0),
            },
            Op::RestoreGraphicsState,
            Op::StartTextSection,
            Op::ShowText {
                items: vec![TextItem::Text("a".to_string())],
            },
            Op::EndTextSection,
        ];
        let (svg, _) = svg_of(ops);
        assert!(svg.contains("font-size=\"20\""), "{svg}");
    }

    #[test]
    fn a_font_size_set_outside_a_save_is_seen_inside_it() {
        let ops = vec![
            Op::SetFont {
                font: PdfFontHandle::External(FontId("F1".to_string())),
                size: Pt(20.0),
            },
            Op::SaveGraphicsState,
            Op::StartTextSection,
            Op::ShowText {
                items: vec![TextItem::Text("a".to_string())],
            },
            Op::EndTextSection,
            Op::RestoreGraphicsState,
        ];
        let (svg, _) = svg_of(ops);
        assert!(svg.contains("font-size=\"20\""), "{svg}");
    }

    // --- the JSON API ---

    #[test]
    fn a_json_document_with_a_mismatched_image_is_saved_and_rendered() {
        use printpdf::wasm::api::{Pdf_DocumentToBytesSync, Pdf_PageToSvgSync};

        for im in [
            image(RawImageFormat::RGB8, 2, 2, 13),
            image(RawImageFormat::RGBA8, 1, 2, 7),
            image(RawImageFormat::RG8, 1, 2, 5),
            grey(usize::MAX, 2, 3),
            grey(2, usize::MAX / 2, 3),
        ] {
            let (mut doc, page) = page_showing(&im);
            doc.with_pages(vec![page.clone()]);
            let save_input = serde_json::json!({ "doc": doc }).to_string();
            let svg_input =
                serde_json::json!({ "page": page, "resources": doc.resources }).to_string();
            let replies = bounded(move || {
                (Pdf_DocumentToBytesSync(save_input), Pdf_PageToSvgSync(svg_input))
            });
            for reply in [replies.0, replies.1] {
                let reply: serde_json::Value =
                    serde_json::from_str(&reply).expect("the API answers with JSON");
                assert_eq!(reply["status"], 0, "{reply}");
            }
        }
    }

    // --- XObjects ---

    #[test]
    fn external_streams_with_odd_content_do_not_panic() {
        let flate_garbage = ExternalStream {
            dict: [(
                "Filter".to_string(),
                DictItem::Name(b"FlateDecode".to_vec()),
            )]
            .into_iter()
            .collect(),
            content: vec![0x78, 0x9C, 0xFF, 0xFF, 0x00],
            compress: false,
        };
        let _ = flate_garbage.decompressed_content();
        let _ = flate_garbage.get_ops();
        for text in [
            "",
            "q",
            "Q Q Q",
            "BT ET",
            "1 0 0 1 re",
            "[ [ [ ] ] ] TJ",
            "<< /A << /B >> >> BDC EMC",
            "(unterminated",
            "99999999999999999999999 0 m",
            "1 2 3 4 5 6 7 8 9 cm",
            "/F1 NaN Tf",
        ] {
            let _ = ExternalStream::decode_ops(text);
        }
    }

    #[test]
    fn dict_items_convert_both_ways() {
        let item = DictItem::Array(vec![
            DictItem::Int(1),
            DictItem::Dict {
                map: [("K".to_string(), DictItem::Null)].into_iter().collect(),
            },
        ]);
        assert_eq!(DictItem::from_lopdf(&item.to_lopdf()), item);
    }
}

#[cfg(feature = "svg")]
mod svg_files {
    use super::*;

    const HEAD: &str = "<svg xmlns='http://www.w3.org/2000/svg'";

    #[test]
    fn svg_that_does_not_parse_is_an_error() {
        let texts = [
            "",
            " ",
            "<",
            "<svg",
            "<svg>",
            "</svg>",
            "not xml at all",
            "<?xml version='1.0'?>",
            "\u{FEFF}",
            "<svg xmlns='http://www.w3.org/2000/svg'><",
            "é<text<text<text",
            "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect",
        ];
        bounded_for(Duration::from_secs(60), move || {
            for text in texts {
                assert!(Svg::parse(text, &mut Vec::new()).is_err(), "{text:?}");
            }
        });
    }

    #[test]
    fn odd_but_well_formed_svg_does_not_panic() {
        let texts = [
            format!("{HEAD}/>"),
            format!("{HEAD} width='0' height='0'/>"),
            format!("{HEAD} width='-5' height='10'/>"),
            format!("{HEAD} width='NaN' height='inf'/>"),
            format!("{HEAD} viewBox='0 0 0 0' width='10' height='10'><rect width='5' height='5'/></svg>"),
            format!("{HEAD} width='10' height='10'><use href='#a'/><g id='a'><use href='#a'/></g></svg>"),
            format!("{HEAD} width='10' height='10'><text>é<text>é</text>é</text><text/></svg>"),
            format!(
                "{HEAD} width='10' height='10'><linearGradient id='g' href='#g'/>\
                 <rect width='5' height='5' fill='url(#g)'/></svg>"
            ),
            format!("{HEAD} width='10' height='10'><image href='data:image/png;base64,AAAA' width='5' height='5'/></svg>"),
            format!("{HEAD} width='10' height='10'><style>*{{</style><rect width='1' height='1'/></svg>"),
        ];
        bounded_for(Duration::from_secs(60), move || {
            for text in &texts {
                let _ = Svg::parse(text, &mut Vec::new());
            }
        });
    }

    #[test]
    fn supplied_fonts_that_are_no_fonts_do_not_panic() {
        let mut fonts = std::collections::BTreeMap::new();
        fonts.insert("empty".to_string(), Vec::new());
        fonts.insert("ttcf".to_string(), b"ttcf\0\x01\0\0\0\0\0\x01\xFF\xFF\xFF\xFF".to_vec());
        fonts.insert("otto".to_string(), b"OTTO\0\xFF\0\0\0\0\0\0\xFF\xFF\xFF\xFF".to_vec());
        fonts.insert("true".to_string(), b"\0\x01\0\0\xFF\xFF".to_vec());
        let svg = format!("{HEAD} width='10' height='10'><text>a</text></svg>");
        bounded_for(Duration::from_secs(60), move || {
            let _ = Svg::parse_with_fonts(&svg, &fonts, &mut Vec::new());
        });
    }

    #[test]
    fn a_plain_svg_still_converts() {
        let svg = format!("{HEAD} width='100' height='50'><rect width='100' height='50' fill='red'/></svg>");
        let x = bounded_for(Duration::from_secs(60), move || Svg::parse(&svg, &mut Vec::new()));
        let x = x.expect("a plain SVG converts");
        assert_eq!(x.width, Some(Px(100)));
    }
}

/// Without the `images` feature pixels are written as 8-bit Flate samples.
#[cfg(not(feature = "images"))]
mod without_images {
    use super::*;

    #[test]
    fn a_huge_size_with_a_tiny_buffer_is_saved() {
        for size in [usize::MAX, usize::MAX / 2 + 1, usize::MAX / 3] {
            for format in ALL_FORMATS {
                bounded(move || save(&image(format, size, 3, 3), &PdfSaveOptions::default()));
                bounded(move || save(&image(format, 3, size, 3), &PdfSaveOptions::default()));
            }
        }
    }

    #[test]
    fn odd_buffers_are_saved() {
        let mut failures = Vec::new();
        for format in ALL_FORMATS {
            for (w, h) in [(0, 0), (1, 1), (2, 3), (3, 1)] {
                for samples in [0, 1, 2, 3, 5, 7, 13, 50] {
                    let im = image(format, w, h, samples);
                    if catch_unwind(AssertUnwindSafe(|| save(&im, &PdfSaveOptions::default()))).is_err() {
                        failures.push(format!("{format:?} {w}x{h} {samples} samples"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "panics on: {failures:#?}");
    }

    #[test]
    fn a_valid_image_is_saved_as_flate_samples() {
        let bytes = save(&image(RawImageFormat::RGB8, 2, 2, 12), &PdfSaveOptions::default());
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn random_images_are_saved() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        let mut failures = Vec::new();
        for round in 0..300 {
            let format = rng.pick(&ALL_FORMATS);
            let (w, h) = (rng.pick(&SIZES), rng.pick(&SIZES));
            let samples = rng.below(40);
            let im = image(format, w, h, samples);
            if catch_unwind(AssertUnwindSafe(|| save(&im, &PdfSaveOptions::default()))).is_err() {
                failures.push(format!("round {round}: {format:?} {w}x{h} {samples} samples"));
            }
        }
        assert!(failures.is_empty(), "panics: {failures:#?}");
    }
}
