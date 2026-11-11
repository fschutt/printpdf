//! Black-and-white pictures (scanned engravings, woodcuts, line art).
//!
//! A greyscale image that is only black and white is written with one bit per pixel,
//! `dither_greyscale` turns greyscale into black and white (Floyd–Steinberg), and
//! `optimize_images` encodes the pictures of a finished PDF again, in place.

#![cfg(feature = "images")]

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use printpdf::*;

const BLACK: u8 = 0;
const WHITE: u8 = 255;

fn grey(width: usize, height: usize, pixels: Vec<u8>) -> RawImage {
    assert_eq!(pixels.len(), width * height);
    RawImage {
        pixels: RawImageData::U8(pixels),
        width,
        height,
        data_format: RawImageFormat::R8,
        tag: Vec::new(),
    }
}

fn pixels(image: &RawImage) -> &[u8] {
    match &image.pixels {
        RawImageData::U8(p) => p,
        _ => panic!("expected 8-bit pixels"),
    }
}

/// 10 x 2: the second byte of each row holds only two pixels.
fn ten_by_two() -> (RawImage, Vec<u8>) {
    let (b, w) = (BLACK, WHITE);
    let image = grey(
        10,
        2,
        vec![
            w, b, w, b, w, b, w, b, w, w, //
            b, b, b, b, b, b, b, b, b, w,
        ],
    );
    // one bit per pixel, 1 = white, each row padded to whole bytes
    (
        image,
        vec![0b1010_1010, 0b1100_0000, 0b0000_0000, 0b0100_0000],
    )
}

/// A noise of greys, the same every run.
fn noise(width: usize, height: usize) -> RawImage {
    let mut state = 0x2545_f491_u32;
    let pixels = (0..width * height)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 24) as u8
        })
        .collect();
    grey(width, height, pixels)
}

fn dithered(mut image: RawImage) -> RawImage {
    image
        .apply_dithering()
        .expect("a greyscale image can be dithered");
    image
}

fn share_of_white(pixels: &[u8]) -> f32 {
    pixels.iter().filter(|&&p| p == WHITE).count() as f32 / pixels.len() as f32
}

fn mean_tone(pixels: &[u8]) -> f32 {
    pixels.iter().map(|&p| p as f32).sum::<f32>() / pixels.len() as f32 / 255.0
}

/// A one-page PDF that shows `image` (and keeps it at the returned object id)
fn pdf_with(image: &RawImage, options: &PdfSaveOptions) -> Vec<u8> {
    let mut doc = PdfDocument::new("bilevel");
    let id = doc.add_image(image);
    let ops = vec![Op::UseXobject {
        id,
        transform: XObjectTransform::default(),
    }];
    doc.with_pages(vec![PdfPage::new(Mm(100.0), Mm(100.0), ops)])
        .save(options, &mut Vec::new())
}

fn is_image(object: &Object) -> bool {
    matches!(object, Object::Stream(s) if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
}

fn stream(doc: &Document, id: ObjectId) -> &Stream {
    doc.get_object(id).unwrap().as_stream().unwrap()
}

fn data(stream: &Stream) -> Vec<u8> {
    if stream.dict.has(b"Filter") {
        stream.decompressed_content().unwrap()
    } else {
        stream.content.clone()
    }
}

fn int(dict: &Dictionary, key: &[u8]) -> i64 {
    dict.get(key).unwrap().as_i64().unwrap()
}

fn name<'a>(dict: &'a Dictionary, key: &[u8]) -> &'a [u8] {
    dict.get(key).unwrap().as_name().unwrap()
}

/// The only image XObject of a PDF
fn the_image(pdf: &[u8]) -> (Dictionary, Vec<u8>) {
    let doc = Document::load_mem(pdf).unwrap();
    let mut images = doc.objects.values().filter(|o| is_image(o));
    let image = images
        .next()
        .expect("an image XObject")
        .as_stream()
        .unwrap();
    assert!(images.next().is_none(), "only one image XObject");
    (image.dict.clone(), data(image))
}

/// A one-page PDF made without printpdf: `add` puts the image XObjects into it, and the page
/// shows each one `add` returns, as /Im0, /Im1, ...
fn pdf_showing(add: impl FnOnce(&mut Document) -> Vec<ObjectId>) -> (Vec<u8>, Vec<ObjectId>) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let ids = add(&mut doc);
    let mut xobjects = Dictionary::new();
    let mut content = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        xobjects.set(format!("Im{i}"), *id);
        content.extend(format!("q 10 0 0 10 {} 0 cm /Im{i} Do Q\n", 10 * i).bytes());
    }
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        "Contents" => content_id,
        "Resources" => dictionary! { "XObject" => xobjects },
    });
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }.into(),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    (bytes, ids)
}

/// An 8-bit DeviceGray image XObject, Flate-compressed as most PDF writers do
fn grey_xobject(image: &RawImage, extra: Dictionary) -> Stream {
    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => image.width as i64,
        "Height" => image.height as i64,
        "ColorSpace" => "DeviceGray",
        "BitsPerComponent" => 8,
    };
    dict.extend(&extra);
    let mut stream = Stream::new(dict, pixels(image).to_vec());
    stream.compress().unwrap();
    stream
}

fn optimized(pdf: &[u8], options: &ImageOptimizationOptions) -> Document {
    let bytes = optimize_images(pdf, options, &mut Vec::new()).expect("optimize_images");
    Document::load_mem(&bytes).unwrap()
}

fn dither() -> ImageOptimizationOptions {
    ImageOptimizationOptions {
        dither_greyscale: Some(true),
        max_image_size: None,
        ..Default::default()
    }
}

// --- writing black-and-white images ---

#[test]
fn a_black_and_white_image_is_written_with_one_bit_per_pixel() {
    let (image, bits) = ten_by_two();
    let (dict, data) = the_image(&pdf_with(&image, &PdfSaveOptions::default()));
    assert_eq!(int(&dict, b"BitsPerComponent"), 1);
    assert_eq!(name(&dict, b"ColorSpace"), b"DeviceGray");
    assert_eq!(data, bits);
}

#[test]
fn a_black_and_white_image_is_written_with_one_bit_per_pixel_uncompressed_too() {
    let (image, bits) = ten_by_two();
    let options = PdfSaveOptions {
        image_optimization: None,
        ..Default::default()
    };
    let (dict, data) = the_image(&pdf_with(&image, &options));
    assert_eq!(int(&dict, b"BitsPerComponent"), 1);
    assert_eq!(data, bits);
}

#[test]
fn a_grey_image_keeps_eight_bits_per_pixel() {
    let image = grey(3, 1, vec![BLACK, 128, WHITE]);
    let (dict, data) = the_image(&pdf_with(&image, &PdfSaveOptions::default()));
    assert_eq!(int(&dict, b"BitsPerComponent"), 8);
    assert_eq!(data, vec![BLACK, 128, WHITE]);
}

#[cfg(feature = "jpeg")]
#[test]
fn a_black_and_white_image_is_not_made_a_jpeg() {
    let (image, bits) = ten_by_two();
    let options = PdfSaveOptions {
        image_optimization: Some(ImageOptimizationOptions {
            format: Some(ImageCompression::Jpeg),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (dict, data) = the_image(&pdf_with(&image, &options));
    assert_eq!(name(&dict, b"Filter"), b"FlateDecode");
    assert_eq!(int(&dict, b"BitsPerComponent"), 1);
    assert_eq!(data, bits);
}

// --- Floyd–Steinberg dithering ---

#[test]
fn dithering_leaves_only_black_and_white() {
    let gradient = grey(256, 16, (0..16).flat_map(|_| 0..=255u8).collect());
    let image = dithered(gradient);
    assert!(pixels(&image).iter().all(|&p| p == BLACK || p == WHITE));
}

#[test]
fn dithering_leaves_a_black_and_white_image_as_it_is() {
    let (image, _) = ten_by_two();
    assert_eq!(pixels(&dithered(image.clone())), pixels(&image));
}

#[test]
fn dithering_keeps_the_tone_of_a_flat_grey() {
    for level in [16u8, 64, 100, 128, 160, 200, 240] {
        let image = dithered(grey(64, 64, vec![level; 64 * 64]));
        let white = share_of_white(pixels(&image));
        let expected = level as f32 / 255.0;
        assert!(
            (white - expected).abs() < 0.02,
            "grey {level}: {:.1}% of the pixels are white, expected {:.1}%",
            white * 100.0,
            expected * 100.0
        );
    }
}

#[test]
fn dithering_keeps_the_tone_of_a_gradient_from_place_to_place() {
    let (width, height) = (256, 64);
    let gradient = grey(width, height, (0..height).flat_map(|_| 0..=255u8).collect());
    let image = dithered(gradient.clone());
    for x0 in (0..width).step_by(16) {
        let band = |p: &[u8]| -> Vec<u8> {
            p.chunks(width)
                .flat_map(|row| row[x0..x0 + 16].to_vec())
                .collect()
        };
        let (was, is) = (
            mean_tone(&band(pixels(&gradient))),
            mean_tone(&band(pixels(&image))),
        );
        assert!(
            (was - is).abs() < 0.03,
            "columns {x0}..{}: tone {is:.3} after dithering, {was:.3} before",
            x0 + 16
        );
    }
}

#[test]
fn a_dithered_image_is_written_with_one_bit_per_pixel() {
    let options = PdfSaveOptions {
        image_optimization: Some(dither()),
        ..Default::default()
    };
    let (dict, data) = the_image(&pdf_with(&noise(64, 64), &options));
    assert_eq!(int(&dict, b"BitsPerComponent"), 1);
    assert_eq!(data.len(), 64 / 8 * 64);
}

#[test]
fn a_grey_image_scaled_down_and_dithered_keeps_its_tone() {
    // 16 KB of pixels, at most 1 KB: scaled down (nearest neighbour) to 32 x 32, then dithered.
    // Dithered first, the scaling would keep every fourth pixel of the dither pattern, which
    // for a flat grey is about a checkerboard: all black or all white.
    let options = PdfSaveOptions {
        image_optimization: Some(ImageOptimizationOptions {
            max_image_size: Some("1kb".to_string()),
            ..dither()
        }),
        ..Default::default()
    };
    let (dict, bits) = the_image(&pdf_with(&grey(128, 128, vec![128; 128 * 128]), &options));
    let (width, height) = (
        int(&dict, b"Width") as usize,
        int(&dict, b"Height") as usize,
    );
    assert!(width < 128, "scaled down");
    assert_eq!(int(&dict, b"BitsPerComponent"), 1);
    let row = width.div_ceil(8);
    let white = (0..height)
        .flat_map(|y| (0..width).map(move |x| (y, x)))
        .filter(|&(y, x)| bits[y * row + x / 8] & (0x80 >> (x % 8)) != 0)
        .count();
    let share = white as f32 / (width * height) as f32;
    assert!(
        (share - 128.0 / 255.0).abs() < 0.05,
        "{:.0}% of the pixels are white, expected 50%",
        share * 100.0
    );
}

// --- optimize_images: a finished PDF, its pictures encoded again in place ---

#[test]
fn optimize_images_writes_a_black_and_white_picture_with_one_bit_per_pixel() {
    let (image, bits) = ten_by_two();
    let extra = dictionary! { "Intent" => "Perceptual" };
    let (pdf, ids) = pdf_showing(|doc| vec![doc.add_object(grey_xobject(&image, extra))]);
    let before = Document::load_mem(&pdf).unwrap();

    let after = optimized(&pdf, &ImageOptimizationOptions::default());
    let new = stream(&after, ids[0]);
    assert_eq!(int(&new.dict, b"BitsPerComponent"), 1);
    assert_eq!(data(new), bits);
    assert_eq!(
        name(&new.dict, b"Intent"),
        b"Perceptual",
        "other entries are kept"
    );

    // the page is untouched, and still shows the picture under the same object number
    let page = |doc: &Document| {
        doc.get_object(*doc.get_pages().get(&1).unwrap())
            .unwrap()
            .clone()
    };
    assert_eq!(page(&after), page(&before));
    let content = |doc: &Document| {
        data(stream(
            doc,
            doc.get_page_contents(*doc.get_pages().get(&1).unwrap())[0],
        ))
    };
    assert_eq!(content(&after), content(&before));
}

#[test]
fn optimize_images_dithers_grey_pictures_when_asked() {
    let (pdf, ids) =
        pdf_showing(|doc| vec![doc.add_object(grey_xobject(&noise(128, 128), Dictionary::new()))]);
    let bytes = optimize_images(&pdf, &dither(), &mut Vec::new()).unwrap();
    assert!(
        bytes.len() < pdf.len(),
        "{} bytes after, {} before",
        bytes.len(),
        pdf.len()
    );

    let after = Document::load_mem(&bytes).unwrap();
    let new = stream(&after, ids[0]);
    assert_eq!(int(&new.dict, b"BitsPerComponent"), 1);
    assert_eq!(data(new).len(), 128 / 8 * 128);
}

#[test]
fn optimize_images_leaves_masked_inverted_and_jpeg_pictures_alone() {
    // all of them black and white with eight bits per pixel, which would be made one bit
    let (image, _) = ten_by_two();
    let mut mask = None;
    let (pdf, shown) = pdf_showing(|doc| {
        let soft_mask = doc.add_object(grey_xobject(&image, Dictionary::new()));
        mask = Some(soft_mask);
        let masked = grey_xobject(&image, dictionary! { "SMask" => soft_mask });
        let inverted = grey_xobject(&image, dictionary! { "Decode" => vec![1.into(), 0.into()] });
        let mut jpeg = grey_xobject(&image, Dictionary::new());
        jpeg.dict.set("Filter", "DCTDecode");
        jpeg.set_content(b"not decoded".to_vec());
        vec![
            doc.add_object(masked),
            doc.add_object(inverted),
            doc.add_object(jpeg),
        ]
    });
    let before = Document::load_mem(&pdf).unwrap();

    let after = optimized(&pdf, &dither());
    for id in shown.into_iter().chain(mask) {
        assert_eq!(stream(&after, id), stream(&before, id), "{id:?} changed");
    }
}
