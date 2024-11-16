use allsorts::subset;
use printpdf::*;

const WIN_1252: &[char; 214] = &[
    '!', '"','#', '$', '%','&', '\'',
	'(', ')', '*', '+', ',', '-', '.', '/',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<',
    '=', '>', '?', '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
    'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', '`', 'a', 'b', 'c',
    'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p',
    'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', 
    '~', '€', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ',
    'Ž', '‘', '’', '“', '•', '–', '—', '˜', '™', 'š', '›', 'œ', 'ž',
    'Ÿ', '¡', '¢', '£', '¤', '¥', '¦', '§', '¨', '©', 'ª', '«', '¬',
    '®', '¯', '°', '±', '²', '³', '´', 'µ', '¶', '·', '¸', '¹', 'º',
    '»', '¼', '½', '¾', '¿', 'À', 'Á', 'Â', 'Ã', 'Ä', 'Å', 'Æ', 'Ç',
    'È', 'É', 'Ê', 'Ë', 'Ì', 'Í', 'Î', 'Ï', 'Ð', 'Ñ', 'Ò', 'Ó', 'Ô', 
    'Õ', 'Ö', '×', 'Ø', 'Ù', 'Ú', 'Û', 'Ü', 'Ý', 'Þ', 'ß', 'à', 'á', 
    'â', 'ã', 'ä', 'å', 'æ', 'ç', 'è', 'é', 'ê', 'ë', 'ì', 'í', 'î', 
    'ï', 'ð', 'ñ', 'ò', 'ó', 'ô', 'õ', 'ö', '÷', 'ø', 'ù', 'ú', 'û', 
    'ü', 'ý', 'þ', 'ÿ'
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
            printpdf::compress(&subset.bytes),
        );
        for (old_gid, (new_gid, char)) in subset.glyph_mapping.iter() {
            target_map.push(format!(
                "    ({}, {old_gid}, {new_gid}, '{c}'),",
                name.get_num(),
                c = if *char == '\'' {
                    "\\'".to_string()
                } else if *char == '\\' {
                    "\\\\".to_string()
                } else {
                    char.to_string()
                }
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
