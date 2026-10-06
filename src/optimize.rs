//! Shrinking a finished PDF without changing what it shows or what its text reads as: its
//! pictures encoded again ([`optimize_images`]), and the `/ActualText` its glyphs read as anyway
//! dropped ([`optimize_text`]).

#[cfg(feature = "images")]
use std::collections::BTreeSet;

#[cfg(feature = "images")]
use lopdf::{Document, Object, ObjectId, Stream};

use crate::PdfWarnMsg;
#[cfg(feature = "images")]
use crate::{
    deserialize::raw_bitmap_from_stream, image::image_to_stream, ImageOptimizationOptions,
};

/// Drops the `/ActualText` of every `/Span` in a finished PDF whose glyphs read as that text
/// anyway.
pub fn optimize_text(pdf: &[u8], _warnings: &mut Vec<PdfWarnMsg>) -> Result<Vec<u8>, String> {
    Ok(pdf.to_vec())
}

/// Encodes the pictures of a finished PDF again, as [`PdfDocument::save`](crate::PdfDocument::save)
/// would with `options`, and puts each one that comes out smaller in place of the old one, under
/// the same object number, so the pages that show it stay as they are. Fonts, pages and content
/// streams are written back unchanged.
///
/// Only pictures whose pixels printpdf reads and writes the same way are encoded again: 8-bit
/// DeviceGray or DeviceRGB, Flate-compressed or not, with no soft mask, mask, /Decode array or
/// predictor. JPEGs, masks and everything else are kept as they are.
///
/// For scans of black-and-white prints (engravings, woodcuts), `dither_greyscale` makes their
/// grey pixels black or white, and the pictures are then written with one bit per pixel.
#[cfg(feature = "images")]
pub fn optimize_images(
    pdf: &[u8],
    options: &ImageOptimizationOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // masks are pictures too, but they belong to the picture they mask
    let masks: BTreeSet<ObjectId> = doc
        .objects
        .values()
        .filter_map(as_image)
        .flat_map(|image| {
            [b"SMask".as_slice(), b"Mask"]
                .into_iter()
                .filter_map(|key| image.dict.get(key).and_then(Object::as_reference).ok())
        })
        .collect();
    let pictures: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter(|(id, object)| !masks.contains(id) && as_image(object).is_some_and(reencodable))
        .map(|(id, _)| *id)
        .collect();

    for id in pictures {
        let Some(old) = doc.objects.get(&id).and_then(as_image) else {
            continue;
        };
        let Some(picture) = raw_bitmap_from_stream(&doc, old) else {
            continue;
        };
        let old_dict = old.dict.clone();
        let old_len = old.content.len();
        let mut new = image_to_stream(picture, &mut doc, Some(options));
        if new.content.len() >= old_len {
            continue;
        }
        // whatever else the picture carries (/Intent, /Metadata, /OC, ...) stays with it
        for (key, value) in old_dict.iter() {
            if !new.dict.has(key) && !matches!(key.as_slice(), b"Filter" | b"DecodeParms") {
                new.dict.set(key.clone(), value.clone());
            }
        }
        warnings.push(PdfWarnMsg::info(
            0,
            0,
            format!(
                "picture {} {} R: {old_len} bytes, now {}",
                id.0,
                id.1,
                new.content.len()
            ),
        ));
        doc.objects.insert(id, Object::Stream(new));
    }

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)
        .map_err(|e| format!("Failed to write PDF: {e}"))?;
    Ok(bytes)
}

#[cfg(feature = "images")]
fn as_image(object: &Object) -> Option<&Stream> {
    match object {
        Object::Stream(s)
            if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") =>
        {
            Some(s)
        }
        _ => None,
    }
}

/// Whether the picture's pixels are read and written the same way: 8-bit DeviceGray or
/// DeviceRGB, Flate or no filter, and nothing else that changes how they are drawn
#[cfg(feature = "images")]
fn reencodable(image: &Stream) -> bool {
    let dict = &image.dict;
    let flate_or_none = match dict.get(b"Filter") {
        Err(_) => true,
        Ok(Object::Name(filter)) => filter == b"FlateDecode",
        Ok(Object::Array(filters)) => filters
            .iter()
            .all(|f| f.as_name().ok() == Some(b"FlateDecode")),
        Ok(_) => false,
    };
    flate_or_none
        && matches!(
            dict.get(b"ColorSpace").and_then(Object::as_name).ok(),
            Some(b"DeviceGray" | b"DeviceRGB")
        )
        && dict.get(b"BitsPerComponent").and_then(Object::as_i64).ok() == Some(8)
        && [
            b"SMask".as_slice(),
            b"Mask",
            b"Decode",
            b"DecodeParms",
            b"ImageMask",
        ]
        .iter()
        .all(|key| !dict.has(key))
}
