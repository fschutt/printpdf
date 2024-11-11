use printpdf::*;

const WIN_1252: &[char; 128] = &[
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    '\u{00A0}', '\u{00A1}', '\u{00A2}', '\u{00A3}', '\u{00A4}', '\u{00A5}', '\u{00A6}', '\u{00A7}',
    '\u{00A8}', '\u{00A9}', '\u{00AA}', '\u{00AB}', '\u{00AC}', '\u{00AD}', '\u{00AE}', '\u{00AF}',
    '\u{00B0}', '\u{00B1}', '\u{00B2}', '\u{00B3}', '\u{00B4}', '\u{00B5}', '\u{00B6}', '\u{00B7}',
    '\u{00B8}', '\u{00B9}', '\u{00BA}', '\u{00BB}', '\u{00BC}', '\u{00BD}', '\u{00BE}', '\u{00BF}',
    '\u{00C0}', '\u{00C1}', '\u{00C2}', '\u{00C3}', '\u{00C4}', '\u{00C5}', '\u{00C6}', '\u{00C7}',
    '\u{00C8}', '\u{00C9}', '\u{00CA}', '\u{00CB}', '\u{00CC}', '\u{00CD}', '\u{00CE}', '\u{00CF}',
    '\u{00D0}', '\u{00D1}', '\u{00D2}', '\u{00D3}', '\u{00D4}', '\u{00D5}', '\u{00D6}', '\u{00D7}',
    '\u{00D8}', '\u{00D9}', '\u{00DA}', '\u{00DB}', '\u{00DC}', '\u{00DD}', '\u{00DE}', '\u{00DF}',
    '\u{00E0}', '\u{00E1}', '\u{00E2}', '\u{00E3}', '\u{00E4}', '\u{00E5}', '\u{00E6}', '\u{00E7}',
    '\u{00E8}', '\u{00E9}', '\u{00EA}', '\u{00EB}', '\u{00EC}', '\u{00ED}', '\u{00EE}', '\u{00EF}',
    '\u{00F0}', '\u{00F1}', '\u{00F2}', '\u{00F3}', '\u{00F4}', '\u{00F5}', '\u{00F6}', '\u{00F7}',
    '\u{00F8}', '\u{00F9}', '\u{00FA}', '\u{00FB}', '\u{00FC}', '\u{00FD}', '\u{00FE}', '\u{00FF}',
];

const FONTS: &[(BuiltinFont, &[u8])] = &[
    (
        BuiltinFont::Courier,
        include_bytes!("./assets/fonts/Courier.ttf"),
    ),
    (
        BuiltinFont::CourierOblique,
        include_bytes!("./assets/fonts/Courier-Oblique.ttf"),
    ),
    (
        BuiltinFont::CourierBold,
        include_bytes!("./assets/fonts/Courier-Bold.ttf"),
    ),
    (
        BuiltinFont::CourierBoldOblique,
        include_bytes!("./assets/fonts/Courier-BoldOblique.ttf"),
    ),
    (
        BuiltinFont::Helvetica,
        include_bytes!("./assets/fonts/Helvetica.ttf"),
    ),
    (
        BuiltinFont::HelveticaBold,
        include_bytes!("./assets/fonts/Helvetica-Bold.ttf"),
    ),
    (
        BuiltinFont::HelveticaOblique,
        include_bytes!("./assets/fonts/Helvetica-Oblique.ttf"),
    ),
    (
        BuiltinFont::HelveticaBoldOblique,
        include_bytes!("./assets/fonts/Helvetica-BoldOblique.ttf"),
    ),
    (
        BuiltinFont::Symbol,
        include_bytes!("./assets/fonts/PDFASymbol.woff2"),
    ),
    (
        BuiltinFont::TimesRoman,
        include_bytes!("./assets/fonts/Times.ttf"),
    ),
    (
        BuiltinFont::TimesBold,
        include_bytes!("./assets/fonts/Times-Bold.ttf"),
    ),
    (
        BuiltinFont::TimesItalic,
        include_bytes!("./assets/fonts/Times-Oblique.ttf"),
    ),
    (
        BuiltinFont::TimesBoldItalic,
        include_bytes!("./assets/fonts/Times-BoldOblique.ttf"),
    ),
    (
        BuiltinFont::ZapfDingbats,
        include_bytes!("./assets/fonts/ZapfDingbats.ttf"),
    ),
];

fn main() {
    let charmap = WIN_1252.iter().copied().collect();
    let mut target_map = vec![];

    for (name, bytes) in FONTS {
        let font = ParsedFont::from_bytes(bytes, 0).unwrap();
        let subset = font.subset_simple(&charmap).unwrap();
        let _ = std::fs::write(
            format!(
                "{}/defaultfonts/{}.subset.ttf",
                env!("CARGO_MANIFEST_DIR"),
                name.get_id()
            ),
            &printpdf::compress(&subset.bytes),
        );
        for (old_gid, (new_gid, char)) in subset.glyph_mapping.iter() {
            target_map.push(format!(
                "    ({}, {old_gid}, {new_gid}, '{char}'),",
                name.get_num()
            ));
        }
    }

    let mut tm = vec![format!(
        "const FONTS: &[(usize, u16, u16, char);{}] = &[",
        target_map.len()
    )];
    tm.append(&mut target_map);
    tm.push("];".to_string());

    let _ = std::fs::write(
        format!("{}/defaultfonts/mapping.rs", env!("CARGO_MANIFEST_DIR")),
        tm.join("\r\n"),
    );
}
