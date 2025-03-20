use std::{
    io::Read,
    sync::atomic::{AtomicUsize, Ordering},
};

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
#[cfg(target_family = "wasm")]
pub(crate) fn to_pdf_time_stamp_metadata(date: &OffsetDateTime) -> String {
    "D:19700101000000+00'00'".to_string()
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn to_pdf_time_stamp_metadata(date: &OffsetDateTime) -> String {
    format!(
        "D:{:04}{:02}{:02}{:02}{:02}{:02}+00'00'",
        date.year(),
        u8::from(date.month()),
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
    )
}

#[cfg(target_family = "wasm")]
pub(crate) fn to_pdf_xmp_date(_date: &OffsetDateTime) -> String {
    "D:1970-01-01T00:00:00+00'00'".to_string()
}

// D:2018-09-19T10:05:05+00'00'
#[cfg(not(target_family = "wasm"))]
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

#[allow(dead_code)]
pub(crate) fn compress(bytes: &[u8]) -> Vec<u8> {
    use std::io::prelude::*;

    use flate2::{write::GzEncoder, Compression};
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    let _ = encoder.write_all(bytes);
    encoder.finish().unwrap_or_default()
}

pub(crate) fn uncompress(bytes: &[u8]) -> Vec<u8> {
    use flate2::read::GzDecoder;
    let mut gz = GzDecoder::new(bytes);
    let mut s = Vec::<u8>::new();
    let _ = gz.read_to_end(&mut s);
    s
}
