use allsorts::{
    binary::read::ReadScope,
    font::read_cmap_subtable,
    font_data::FontData,
    tables::{cmap::Cmap, FontTableProvider},
    tag,
};
use std::collections::{HashMap, HashSet};

pub(crate) fn subset(
    font_bytes: &[u8],
    used_glyphs: &mut HashSet<u16>,
) -> (Vec<u8>, HashMap<u16, u16>) {
    let font_file = ReadScope::new(font_bytes).read::<FontData<'_>>().unwrap();
    let provider = font_file.table_provider(0).unwrap();
    let cmap_data = provider.read_table_data(tag::CMAP).unwrap();
    let cmap = ReadScope::new(&cmap_data).read::<Cmap<'_>>().unwrap();
    let (_, cmap_subtable) = read_cmap_subtable(&cmap).unwrap().unwrap();

    // Prevent `allsorts` from using MacRoman encoding by using a non supported character
    let gid_eur = cmap_subtable.map_glyph('€' as u32).unwrap().unwrap();
    used_glyphs.insert(0);
    used_glyphs.insert(gid_eur);

    let mut glyph_ids: Vec<u16> = used_glyphs.iter().copied().collect();

    glyph_ids.sort_unstable();

    let mut gid_char_map = Vec::new();
    cmap_subtable
        .mappings_fn(|ch, gid| {
            if used_glyphs.contains(&gid) {
                gid_char_map.push((gid, ch));
            }
        })
        .unwrap();

    let new_font = allsorts::subset::subset(&provider, &glyph_ids).unwrap();

    let font_file = ReadScope::new(&new_font).read::<FontData<'_>>().unwrap();
    let provider = font_file.table_provider(0).unwrap();
    let cmap_data = provider.read_table_data(tag::CMAP).unwrap();
    let cmap = ReadScope::new(&cmap_data).read::<Cmap<'_>>().unwrap();
    let (_, cmap_subtable) = read_cmap_subtable(&cmap).unwrap().unwrap();

    let mut gid_mapping = HashMap::new();
    for (old_gid, ch) in gid_char_map {
        let new_gid = cmap_subtable.map_glyph(ch).unwrap().unwrap();
        gid_mapping.insert(old_gid, new_gid);
    }

    drop(provider);
    (new_font, gid_mapping)
}
