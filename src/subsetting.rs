use allsorts::{
    binary::read::ReadScope,
    font::read_cmap_subtable,
    font_data::FontData,
    tables::{cmap::Cmap, FontTableProvider},
    tag,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

pub(crate) struct FontSubset {
    pub(crate) new_font_bytes: Vec<u8>,
    /// Mapping from old GIDs (in the original font) to the new GIDs (in the new subset font)
    pub(crate) gid_mapping: HashMap<u16, u16>,
}

pub(crate) fn subset(
    font_bytes: &[u8],
    used_glyphs: &mut HashSet<u16>,
) -> Result<FontSubset, Box<dyn Error>> {
    let font_file = ReadScope::new(font_bytes).read::<FontData<'_>>()?;
    let provider = font_file.table_provider(0)?;
    let cmap_data = provider.read_table_data(tag::CMAP)?;
    let cmap = ReadScope::new(&cmap_data).read::<Cmap<'_>>()?;
    let (_, cmap_subtable) =
        read_cmap_subtable(&cmap)?.ok_or(allsorts::error::ParseError::MissingValue)?;

    let mut non_macroman_char = None;

    // Try to find a char this is not encoded in the MacRoman encoding
    cmap_subtable.mappings_fn(|chr, _gid| {
        if non_macroman_char.is_none() && !allsorts::macroman::is_macroman(char::from_u32(chr).unwrap_or('\0')) {
            non_macroman_char = Some(chr);
        }
    })?;

    let non_macroman_gid = cmap_subtable
        .map_glyph(non_macroman_char.ok_or(allsorts::error::ParseError::MissingValue)?)?
        .ok_or(allsorts::error::ParseError::MissingValue)?;

    used_glyphs.insert(0);

    // Prevent `allsorts` from using MacRoman encoding by using a non supported character since the
    // MacRoman encoding doesn't seem to work in PDFs
    used_glyphs.insert(non_macroman_gid);

    let mut glyph_ids: Vec<u16> = used_glyphs.iter().copied().collect();

    glyph_ids.sort_unstable();

    let new_font_bytes = allsorts::subset::subset(&provider, &glyph_ids)?;

    let mut gid_mapping = HashMap::new();
    for (idx, old_gid) in glyph_ids.into_iter().enumerate() {
        gid_mapping.insert(old_gid, idx as u16);
    }

    Ok(FontSubset {
        new_font_bytes,
        gid_mapping,
    })
}
