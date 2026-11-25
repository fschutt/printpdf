//! Embedding the standard fonts, for the standards that require every font to be embedded.
//!
//! The 14 standard fonts are normally written as a name only: `/Type1 /Helvetica` with
//! `WinAnsiEncoding`, and the viewer supplies the glyphs. PDF/A and PDF/X need the glyphs in the
//! file. For these standards the font is written as a simple TrueType font instead, with the same
//! resource name, the same `WinAnsiEncoding` and the same one-byte codes in the content stream, so
//! the text is the same for a viewer that extracts it. Only the font dictionary changes.
//!
//! The glyphs come from the font files that printpdf bundles (`BuiltinFont::get_subset_font`),
//! cut down to the glyphs that the document uses when the `text_layout` feature is on. The
//! bundled files carry a Unicode `cmap` of platform 0. A simple TrueType font with an encoding
//! needs one of platform 3, so the `cmap` is written again, with the characters that the document
//! uses.
//!
//! `Symbol` and `ZapfDingbats` are not embedded. The bundled files for them have a few dozen
//! glyphs at Latin code points, not the glyph set that the standard font has, so embedding them
//! would show other symbols than the viewer's Symbol does. The save warns.

use std::collections::{BTreeMap, BTreeSet};

use lopdf::{
    Dictionary as LoDictionary,
    Object::{Array, Integer, Name, Reference},
};

use super::{font_program_stream, win_ansi_char};
use crate::{BuiltinFont, PdfWarnMsg};

/// `FontDescriptor` values of the standard fonts that the font files do not carry (there is no
/// OS/2 table in the bundled files): cap height, italic angle, vertical stem, serif, fixed pitch.
/// They are the values of the Adobe font metrics of the standard fonts.
fn descriptor_values(font: &BuiltinFont) -> (i64, i64, i64, bool, bool) {
    use BuiltinFont::*;
    match font {
        TimesRoman => (662, 0, 84, true, false),
        TimesBold => (676, 0, 139, true, false),
        TimesItalic => (653, -16, 76, true, false),
        TimesBoldItalic => (669, -15, 121, true, false),
        Helvetica => (718, 0, 88, false, false),
        HelveticaBold => (718, 0, 140, false, false),
        HelveticaOblique => (718, -12, 88, false, false),
        HelveticaBoldOblique => (718, -12, 140, false, false),
        Courier => (562, 0, 51, true, true),
        CourierBold => (562, 0, 106, true, true),
        CourierOblique => (562, -12, 51, true, true),
        CourierBoldOblique => (562, -12, 106, true, true),
        Symbol | ZapfDingbats => (700, 0, 80, false, false),
    }
}

/// The font dictionary of `font` with its font program embedded, for the text that uses `codes`
/// (WinAnsi codes). `None` when the font cannot be embedded; the caller then writes the plain
/// standard font, and a warning says why.
pub(super) fn embedded_font_dict(
    doc: &mut lopdf::Document,
    font: &BuiltinFont,
    codes: &BTreeSet<u8>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Option<LoDictionary> {
    let name = font.get_id();
    if matches!(font, BuiltinFont::Symbol | BuiltinFont::ZapfDingbats) {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!(
                "built-in font {name} is not embedded: the font file that printpdf bundles for it \
                 does not have the glyphs of the standard font. Embed a font of your own."
            ),
        ));
        return None;
    }

    // The bundled font: its glyph for each character. The table of printpdf lists the glyphs of
    // the file (glyph 0 is the exclamation mark: the files have no `.notdef` of their own), and
    // the character map of the file adds the space and a few more.
    let bundled = font.get_subset_font();
    let mut glyph_of_char: BTreeMap<char, u16> = bundled
        .glyph_mapping
        .values()
        .filter_map(|(gid, text)| {
            let mut chars = text.chars();
            let c = chars.next()?;
            chars.next().is_none().then_some((c, *gid))
        })
        .collect();
    if let Some(cmap) = read_tables(&bundled.bytes).and_then(|tables| {
        tables
            .into_iter()
            .find(|(tag, _)| tag == b"cmap")
            .map(|(_, data)| read_cmap(&data))
    }) {
        for (unicode, gid) in cmap {
            if let Some(c) = char::from_u32(u32::from(unicode)) {
                glyph_of_char.entry(c).or_insert(gid);
            }
        }
    }
    let space = glyph_of_char.get(&' ').copied();

    // The characters the glyphs are looked up by. Code 0xA0 is the glyph `space` and 0xAD the
    // glyph `hyphen` in WinAnsiEncoding, which a viewer finds under U+0020 and U+002D. A character
    // that the bundled font lacks is drawn as a space and named in a warning.
    let mut used: BTreeMap<u8, (char, Option<u16>)> = BTreeMap::new();
    let mut missing: Vec<char> = Vec::new();
    let mut wanted_codes = codes.clone();
    if wanted_codes.is_empty() {
        wanted_codes.insert(b' ');
    }
    for &code in &wanted_codes {
        let c = match code {
            0xA0 => ' ',
            0xAD => '-',
            other => match win_ansi_char(other) {
                Some(c) => c,
                None => continue,
            },
        };
        let gid = glyph_of_char.get(&c).copied();
        if gid.is_none() && !missing.contains(&c) {
            missing.push(c);
        }
        used.insert(code, (c, gid.or(space)));
    }
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|c| format!("'{}' (U+{:04X})", c, u32::from(*c)))
            .collect::<Vec<_>>()
            .join(", ");
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!(
                "built-in font {name}: the font file that printpdf bundles has no glyph for {list}. \
                 The text is kept, so text extraction reads it, but the page draws a space. Embed \
                 a font of your own to draw these characters."
            ),
        ));
    }

    // Cut the font down to the used glyphs, or keep the bundled file as it is.
    let wanted_glyphs: BTreeMap<u16, String> = used
        .values()
        .filter_map(|(c, gid)| gid.map(|g| (g, c.to_string())))
        .collect();
    let (program, new_gid, was_subset) = match subset_program(&bundled.bytes, &wanted_glyphs) {
        Some((bytes, map)) => (bytes, map, true),
        None => (
            bundled.bytes.clone(),
            wanted_glyphs.keys().map(|g| (*g, *g)).collect(),
            false,
        ),
    };

    // The glyph of each code in the program that is written.
    let code_glyph = |code: u8| -> u16 {
        used.get(&code)
            .and_then(|(_, gid)| *gid)
            .and_then(|gid| new_gid.get(&gid).copied())
            .unwrap_or(0)
    };
    let unicode_to_gid: BTreeMap<u16, u16> = used
        .iter()
        .filter_map(|(code, (c, gid))| {
            gid.as_ref()?;
            let unicode = u16::try_from(u32::from(*c)).ok()?;
            Some((unicode, code_glyph(*code)))
        })
        .collect();

    let Some(tables) = read_tables(&program) else {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!("built-in font {name} is not embedded: its font file cannot be read."),
        ));
        return None;
    };
    let metrics = Metrics::read(&tables);
    let Some(metrics) = metrics else {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!("built-in font {name} is not embedded: its font file has no metrics."),
        ));
        return None;
    };
    let cmap = cmap_table(&unicode_to_gid);
    let program = write_tables(&tables, &cmap);

    let first = used.keys().next().copied().unwrap_or(b' ');
    let last = used.keys().next_back().copied().unwrap_or(b' ');
    let widths: Vec<lopdf::Object> = (first..=last)
        .map(|code| {
            if used.contains_key(&code) {
                Integer(metrics.width(code_glyph(code)))
            } else {
                Integer(0)
            }
        })
        .collect();

    let base_font = if was_subset {
        format!("{}+{}", subset_tag(font, &used), name)
    } else {
        name.to_string()
    };

    let (cap_height, italic_angle, stem_v, serif, fixed) = descriptor_values(font);
    let mut flags = 1 << 5; // nonsymbolic: the character set of WinAnsiEncoding
    if serif {
        flags |= 1 << 1;
    }
    if fixed {
        flags |= 1;
    }
    if italic_angle != 0 {
        flags |= 1 << 6;
    }
    let font_file = doc.add_object(lopdf::Object::Stream(font_program_stream(
        LoDictionary::new(),
        program,
        true,
    )));
    let descriptor = doc.add_object(LoDictionary::from_iter(vec![
        ("Type", Name("FontDescriptor".into())),
        ("FontName", Name(base_font.clone().into_bytes())),
        ("Flags", Integer(flags)),
        (
            "FontBBox",
            Array(metrics.bbox.iter().map(|v| Integer(*v)).collect()),
        ),
        ("ItalicAngle", Integer(italic_angle)),
        ("Ascent", Integer(metrics.ascent)),
        ("Descent", Integer(metrics.descent)),
        ("CapHeight", Integer(cap_height)),
        ("StemV", Integer(stem_v)),
        ("FontFile2", Reference(font_file)),
    ]));

    Some(LoDictionary::from_iter(vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name("TrueType".into())),
        ("BaseFont", Name(base_font.into_bytes())),
        ("Encoding", Name("WinAnsiEncoding".into())),
        ("FirstChar", Integer(i64::from(first))),
        ("LastChar", Integer(i64::from(last))),
        ("Widths", Array(widths)),
        ("FontDescriptor", Reference(descriptor)),
    ]))
}

/// Cuts the bundled font file down to `wanted` (glyph to its text). Returns the font program and
/// the new glyph id of each kept glyph. `None` when the font is not cut down: without the
/// `text_layout` feature, or when the cut fails.
#[cfg(feature = "text_layout")]
fn subset_program(
    bundled: &[u8],
    wanted: &BTreeMap<u16, String>,
) -> Option<(Vec<u8>, BTreeMap<u16, u16>)> {
    let parsed = crate::font::ParsedFont::from_bytes(bundled, 0, &mut Vec::new())?;
    let subset = crate::font::subset_font(&parsed, wanted).ok()?;
    let map = subset
        .glyph_mapping
        .iter()
        .map(|(old, (new, _))| (*old, *new))
        .collect();
    Some((subset.bytes, map))
}

#[cfg(not(feature = "text_layout"))]
fn subset_program(
    _bundled: &[u8],
    _wanted: &BTreeMap<u16, String>,
) -> Option<(Vec<u8>, BTreeMap<u16, u16>)> {
    None
}

/// Six capital letters from the font and the codes, for the `ABCDEF+` prefix of a subset's name.
fn subset_tag(font: &BuiltinFont, used: &BTreeMap<u8, (char, Option<u16>)>) -> String {
    // FNV-1a
    let mut hash: u32 = 0x811C_9DC5;
    let mut feed = |byte: u8| {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    };
    feed(font.get_num() as u8);
    for code in used.keys() {
        feed(*code);
    }
    (0..6)
        .map(|i| {
            let letter = (hash >> (i * 5)) % 26;
            char::from(b'A'.saturating_add(letter as u8))
        })
        .collect()
}

pub(super) fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at.checked_add(1)?)?]))
}

fn be_i16(bytes: &[u8], at: usize) -> Option<i16> {
    be16(bytes, at).map(|v| v as i16)
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]))
}

/// The tables of a TrueType font file, by tag, in the order of the table directory.
pub(super) fn read_tables(sfnt: &[u8]) -> Option<Vec<([u8; 4], Vec<u8>)>> {
    let count = usize::from(be16(sfnt, 4)?);
    let mut tables = Vec::new();
    for i in 0..count {
        let record = 12usize.checked_add(i.checked_mul(16)?)?;
        let tag: [u8; 4] = sfnt.get(record..record.checked_add(4)?)?.try_into().ok()?;
        let offset = usize::try_from(be32(sfnt, record.checked_add(8)?)?).ok()?;
        let length = usize::try_from(be32(sfnt, record.checked_add(12)?)?).ok()?;
        let data = sfnt.get(offset..offset.checked_add(length)?)?;
        tables.push((tag, data.to_vec()));
    }
    Some(tables)
}

/// The glyph of each BMP character in a `cmap` table: the Windows Unicode subtable if there is
/// one, else a Unicode subtable. Only subtables of format 4 are read; anything else is empty.
fn read_cmap(cmap: &[u8]) -> BTreeMap<u16, u16> {
    let mut map = BTreeMap::new();
    let Some(count) = be16(cmap, 2) else {
        return map;
    };
    let mut chosen: Option<usize> = None;
    for i in 0..usize::from(count) {
        let Some(record) = i.checked_mul(8).and_then(|o| o.checked_add(4)) else {
            break;
        };
        let (Some(platform), Some(encoding), Some(offset)) = (
            be16(cmap, record),
            be16(cmap, record.saturating_add(2)),
            be32(cmap, record.saturating_add(4)).and_then(|o| usize::try_from(o).ok()),
        ) else {
            continue;
        };
        if be16(cmap, offset) != Some(4) {
            continue;
        }
        if platform == 3 && encoding == 1 {
            chosen = Some(offset);
            break;
        }
        if platform == 0 && chosen.is_none() {
            chosen = Some(offset);
        }
    }
    let Some(sub) = chosen else {
        return map;
    };
    let Some(seg_count) = be16(cmap, sub.saturating_add(6)).map(|v| usize::from(v) / 2) else {
        return map;
    };
    let ends = sub.saturating_add(14);
    let starts = sub.saturating_add(16).saturating_add(seg_count * 2);
    let deltas = starts.saturating_add(seg_count * 2);
    let offsets = deltas.saturating_add(seg_count * 2);
    for i in 0..seg_count {
        let (Some(end), Some(start), Some(delta), Some(range_offset)) = (
            be16(cmap, ends.saturating_add(i * 2)),
            be16(cmap, starts.saturating_add(i * 2)),
            be16(cmap, deltas.saturating_add(i * 2)),
            be16(cmap, offsets.saturating_add(i * 2)),
        ) else {
            break;
        };
        for code in start..=end {
            if code == 0xFFFF {
                continue;
            }
            let gid = if range_offset == 0 {
                code.wrapping_add(delta)
            } else {
                let at = offsets
                    .saturating_add(i * 2)
                    .saturating_add(usize::from(range_offset))
                    .saturating_add(usize::from(code - start) * 2);
                match be16(cmap, at) {
                    Some(0) | None => 0,
                    Some(g) => g.wrapping_add(delta),
                }
            };
            if gid != 0 {
                map.insert(code, gid);
            }
        }
    }
    map
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        for (slot, byte) in word.iter_mut().zip(chunk) {
            *slot = *byte;
        }
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

/// The font file with `tables`, but with `cmap` as its character map and no other `cmap`.
fn write_tables(tables: &[([u8; 4], Vec<u8>)], cmap: &[u8]) -> Vec<u8> {
    let mut list: Vec<(&[u8; 4], Vec<u8>)> = tables
        .iter()
        .filter(|(tag, _)| tag != b"cmap")
        .map(|(tag, data)| (tag, data.clone()))
        .collect();
    list.push((b"cmap", cmap.to_vec()));
    list.sort_by(|a, b| a.0.cmp(b.0));

    // The adjustment of the whole file is stored in `head`; its table checksum counts it as 0.
    for (tag, data) in list.iter_mut() {
        if *tag == b"head" {
            for byte in data.iter_mut().skip(8).take(4) {
                *byte = 0;
            }
        }
    }

    let count = list.len() as u16;
    let mut entry_selector = 0u16;
    while 1u32 << (entry_selector + 1) <= u32::from(count) {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 16;
    let range_shift = count * 16 - search_range;

    let mut offset = 12 + 16 * list.len();
    let mut directory = Vec::new();
    let mut body = Vec::new();
    let mut head_at = None;
    for (tag, data) in &list {
        if *tag == b"head" {
            head_at = Some(offset);
        }
        directory.extend_from_slice(*tag);
        directory.extend_from_slice(&checksum(data).to_be_bytes());
        directory.extend_from_slice(&(offset as u32).to_be_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        let padded = (data.len() + 3) & !3;
        body.resize(body.len() + padded - data.len(), 0);
        offset += padded;
    }

    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    out.extend_from_slice(&directory);
    out.extend_from_slice(&body);

    if let Some(head_at) = head_at {
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        if let Some(slot) = out.get_mut(head_at.saturating_add(8)..head_at.saturating_add(12)) {
            slot.copy_from_slice(&adjustment.to_be_bytes());
        }
    }
    out
}

/// A `cmap` table with one subtable, format 4, platform 3 (Windows), encoding 1 (Unicode BMP).
fn cmap_table(map: &BTreeMap<u16, u16>) -> Vec<u8> {
    // (start, end, delta): runs of consecutive characters with consecutive glyphs
    let mut segments: Vec<(u16, u16, u16)> = Vec::new();
    for (&code, &gid) in map {
        if code == 0xFFFF {
            continue;
        }
        let delta = gid.wrapping_sub(code);
        match segments.last_mut() {
            Some((_, end, d)) if end.checked_add(1) == Some(code) && *d == delta => *end = code,
            _ => segments.push((code, code, delta)),
        }
    }
    segments.push((0xFFFF, 0xFFFF, 1));

    let seg_count = segments.len() as u16;
    let mut entry_selector = 0u16;
    while 1u32 << (entry_selector + 1) <= u32::from(seg_count) {
        entry_selector += 1;
    }
    let search_range = 2 * (1u16 << entry_selector);
    let length = 16 + 8 * segments.len();

    let mut sub = Vec::with_capacity(length);
    sub.extend_from_slice(&4u16.to_be_bytes());
    sub.extend_from_slice(&(length as u16).to_be_bytes());
    sub.extend_from_slice(&0u16.to_be_bytes()); // language
    sub.extend_from_slice(&(seg_count * 2).to_be_bytes());
    sub.extend_from_slice(&search_range.to_be_bytes());
    sub.extend_from_slice(&entry_selector.to_be_bytes());
    sub.extend_from_slice(&(seg_count * 2 - search_range).to_be_bytes());
    for s in &segments {
        sub.extend_from_slice(&s.1.to_be_bytes());
    }
    sub.extend_from_slice(&0u16.to_be_bytes()); // reservedPad
    for s in &segments {
        sub.extend_from_slice(&s.0.to_be_bytes());
    }
    for s in &segments {
        sub.extend_from_slice(&s.2.to_be_bytes());
    }
    for _ in &segments {
        sub.extend_from_slice(&0u16.to_be_bytes()); // idRangeOffset
    }

    let mut table = Vec::with_capacity(12 + sub.len());
    table.extend_from_slice(&0u16.to_be_bytes()); // version
    table.extend_from_slice(&1u16.to_be_bytes()); // numTables
    table.extend_from_slice(&3u16.to_be_bytes()); // platformID
    table.extend_from_slice(&1u16.to_be_bytes()); // encodingID
    table.extend_from_slice(&12u32.to_be_bytes()); // offset of the subtable
    table.extend_from_slice(&sub);
    table
}

/// The numbers of the font file that the font descriptor and `/Widths` need, in 1/1000 em.
struct Metrics {
    units_per_em: i64,
    advances: Vec<u16>,
    ascent: i64,
    descent: i64,
    bbox: [i64; 4],
}

impl Metrics {
    fn read(tables: &[([u8; 4], Vec<u8>)]) -> Option<Metrics> {
        let table = |tag: &[u8; 4]| {
            tables
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, data)| data.as_slice())
        };
        let head = table(b"head")?;
        let hhea = table(b"hhea")?;
        let hmtx = table(b"hmtx")?;
        let units_per_em = i64::from(be16(head, 18)?);
        if units_per_em == 0 {
            return None;
        }
        let scale = |v: i16| (i64::from(v) * 1000 + units_per_em / 2).div_euclid(units_per_em);
        let bbox = [
            scale(be_i16(head, 36)?),
            scale(be_i16(head, 38)?),
            scale(be_i16(head, 40)?),
            scale(be_i16(head, 42)?),
        ];
        let long_metrics = usize::from(be16(hhea, 34)?);
        let mut advances = Vec::with_capacity(long_metrics);
        for i in 0..long_metrics {
            advances.push(be16(hmtx, i.checked_mul(4)?)?);
        }
        Some(Metrics {
            units_per_em,
            advances,
            ascent: scale(be_i16(hhea, 4)?),
            descent: scale(be_i16(hhea, 6)?),
            bbox,
        })
    }

    /// The advance of `gid` in 1/1000 em. A glyph past the last long metric repeats the last one.
    fn width(&self, gid: u16) -> i64 {
        let advance = self
            .advances
            .get(usize::from(gid))
            .or_else(|| self.advances.last())
            .copied()
            .unwrap_or(0);
        (i64::from(advance) * 1000 + self.units_per_em / 2).div_euclid(self.units_per_em)
    }
}
