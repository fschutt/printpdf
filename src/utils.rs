
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::date::OffsetDateTime;

/// Since the random number generator doesn't have to be cryptographically secure
/// it doesn't make sense to import the entire rand library, so this is just a
/// xorshift pseudo-random function
static RAND_SEED: AtomicUsize = AtomicUsize::new(2100);

/// Xorshift-based random number generator. Impure function
pub(crate) fn random_number() -> usize {
    let mut x = RAND_SEED.fetch_add(21, Ordering::SeqCst);
    #[cfg(target_pointer_width = "64")]
    {
        x ^= x << 21;
        x ^= x >> 35;
        x ^= x << 4;
        x
    }

    #[cfg(target_pointer_width = "32")]
    {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    }
}

/// Returns a string with 32 random characters
pub(crate) fn random_character_string_32() -> String {
    const MAX_CHARS: usize = 32;
    let mut final_string = String::with_capacity(MAX_CHARS);
    let mut char_pos = 0;

    'outer: while char_pos < MAX_CHARS {
        let rand = format!("{}", crate::utils::random_number());
        for ch in rand.chars() {
            if char_pos < MAX_CHARS {
                final_string.push(u8_to_char(ch.to_digit(10).unwrap() as u8));
                char_pos += 1;
            } else {
                break 'outer;
            }
        }
    }

    final_string
}

// D:20170505150224+02'00'
pub(crate) fn to_pdf_time_stamp_metadata(date: &OffsetDateTime) -> String {
    let offset = date.offset();
    let offset_sign = if offset.is_negative() { '-' } else { '+' };
    format!(
        "D:{:04}{:02}{:02}{:02}{:02}{:02}{offset_sign}{:02}'{:02}'",
        date.year(),
        u8::from(date.month()),
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
        offset.whole_hours().abs(),
        offset.minutes_past_hour().abs(),
    )
}

// D:2018-09-19T10:05:05+00'00'
pub(crate) fn to_pdf_xmp_date(date: &OffsetDateTime) -> String {
    // Since the time is in UTC, we know that the time zone
    // difference to UTC is 0 min, 0 sec, hence the 00'00
    format!(
        "D:{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00'00'",
        date.year(),
        date.month(),
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
    )
}

/// `0 => A`, `1 => B`, and so on
#[inline(always)]
fn u8_to_char(input: u8) -> char {
    (b'A' + input) as char
}

#[cfg(any(debug_assertions, feature = "less-optimization"))]
#[inline]
pub fn compress_stream(stream: lopdf::Stream) -> lopdf::Stream {
    stream
}

#[cfg(all(not(debug_assertions), not(feature = "less-optimization")))]
#[inline]
pub fn compress_stream(mut stream: lopdf::Stream) -> lopdf::Stream {
    let _ = stream.compress();
    stream
}

#[cfg(feature = "images")]
fn preprocess_image_with_alpha(
    color_type: ColorType,
    image_data: Vec<u8>,
    dim: (u32, u32),
) -> (ImageXObject, Option<ImageXObject>) {
    let (color_type, image_data, smask_data) = match color_type {
        ColorType::Rgba8 => {
            let (rgb, alpha) = rgba_to_rgb(image_data);
            (ColorType::Rgb8, rgb, Some(alpha))
        }
        _ => (color_type, image_data, None),
    };
    let color_bits = ColorBits::from(color_type);
    let color_space = ColorSpace::from(color_type);

    let img = ImageXObject {
        width: Px(dim.0 as usize),
        height: Px(dim.1 as usize),
        color_space,
        bits_per_component: color_bits,
        image_data,
        interpolate: true,
        image_filter: None,
        clipping_bbox: None,
        smask: None,
    };
    let img_mask = smask_data.map(|smask| ImageXObject {
        width: img.width,
        height: img.height,
        color_space: ColorSpace::Greyscale,
        bits_per_component: ColorBits::Bit8,
        interpolate: false,
        image_data: smask,
        image_filter: None,
        clipping_bbox: None,
        smask: None,
    });
    (img, img_mask)
}
