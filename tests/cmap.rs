#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use lopdf::{Object, StringFormat};
    use printpdf::{
        cmap::ToUnicodeCMap,
        text::{decode_pdf_string, decode_tj_operands},
    };

    #[test]
    fn test_to_unicode_cmap_parsing() {
        // The CMap data from the PDF file
        let cmap_data = r#"
/CIDInit /ProcSet findresource begin

12 dict begin

begincmap

%!PS-Adobe-3.0 Resource-CMap
%%DocumentNeededResources: procset CIDInit
%%IncludeResource: procset CIDInit

/CIDSystemInfo 3 dict dup begin
    /Registry (FontSpecific) def
    /Ordering (HEIGIDGCBAAHFGBHAEFHCBHGAJHCJDHF) def
    /Supplement 0 def
end def

/CMapName /FontSpecific-HEIGIDGCBAAHFGBHAEFHCBHGAJHCJDHF def
/CMapVersion 1 def
/CMapType 2 def
/WMode 0 def

1 begincodespacerange
<0000> 
endcodespacerange
13 beginbfchar
<0000> <0020>
<0001> <002c>
<0002> <003f>
<0003> <0432>
<0004> <0434>
<0005> <0438>
<0006> <043a>
<0007> <043b>
<0008> <0442>
<0009> <041f>
<000a> <0430>
<000b> <0435>
<000c> <0440>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
        "#;

        // Parse the CMap
        let cmap = ToUnicodeCMap::parse(cmap_data).expect("Failed to parse CMap");

        // Verify the mappings are correct
        assert_eq!(cmap.mappings.len(), 13, "Expected 13 mappings");

        // Check a few specific mappings
        assert_eq!(cmap.mappings.get(&0x0000), Some(&vec![0x0020])); // space
        assert_eq!(cmap.mappings.get(&0x0001), Some(&vec![0x002c])); // comma
        assert_eq!(cmap.mappings.get(&0x0009), Some(&vec![0x041f])); // Cyrillic 'П'
        assert_eq!(cmap.mappings.get(&0x000a), Some(&vec![0x0430])); // Cyrillic 'а'

        // Test decoding the actual content from the PDF
        let bytes = [
            0x00, 0x09, 0x00, 0x0c, 0x00, 0x05, 0x00, 0x03, 0x00, 0x0b, 0x00, 0x08, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x06, 0x00, 0x0a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x04, 0x00, 0x0b,
            0x00, 0x07, 0x00, 0x0a, 0x00, 0x02,
        ];

        // Create a PDFString object
        let pdf_string = Object::String(bytes.to_vec(), StringFormat::Literal);

        // Decode the string using the CMap
        let decoded = decode_pdf_string(&pdf_string, Some(&cmap));

        // The expected result
        let expected = "Привет, как дела?";

        // Verify the result
        assert_eq!(decoded, expected, "Failed to decode PDF string using CMap");
    }

    #[test]
    fn test_manual_cmap_decoding() {
        // Create a manual mapping
        let mut mappings = BTreeMap::new();
        mappings.insert(0x0000, vec![0x0020]); // space
        mappings.insert(0x0001, vec![0x002c]); // comma
        mappings.insert(0x0002, vec![0x003f]); // question mark
        mappings.insert(0x0003, vec![0x0432]); // Cyrillic 'в'
        mappings.insert(0x0004, vec![0x0434]); // Cyrillic 'д'
        mappings.insert(0x0005, vec![0x0438]); // Cyrillic 'и'
        mappings.insert(0x0006, vec![0x043a]); // Cyrillic 'к'
        mappings.insert(0x0007, vec![0x043b]); // Cyrillic 'л'
        mappings.insert(0x0008, vec![0x0442]); // Cyrillic 'т'
        mappings.insert(0x0009, vec![0x041f]); // Cyrillic 'П'
        mappings.insert(0x000a, vec![0x0430]); // Cyrillic 'а'
        mappings.insert(0x000b, vec![0x0435]); // Cyrillic 'е'
        mappings.insert(0x000c, vec![0x0440]); // Cyrillic 'р'

        let cmap = ToUnicodeCMap { mappings };

        // Manual decoding function
        fn decode_with_cmap(bytes: &[u8], cmap: &ToUnicodeCMap) -> String {
            let mut result = String::new();
            let mut i = 0;

            while i < bytes.len() {
                if i + 1 < bytes.len() {
                    // Process as 2-byte CID
                    let cid = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);

                    if let Some(unis) = cmap.mappings.get(&cid) {
                        // Convert Unicode values to characters
                        for &u in unis {
                            if let Some(c) = std::char::from_u32(u) {
                                result.push(c);
                            }
                        }
                    }
                    i += 2;
                } else {
                    // Handle odd byte at the end
                    i += 1;
                }
            }

            result
        }

        // The actual content from the PDF
        let bytes = [
            0x00, 0x09, 0x00, 0x0c, 0x00, 0x05, 0x00, 0x03, 0x00, 0x0b, 0x00, 0x08, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x06, 0x00, 0x0a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x04, 0x00, 0x0b,
            0x00, 0x07, 0x00, 0x0a, 0x00, 0x02,
        ];

        let decoded = decode_with_cmap(&bytes, &cmap);
        let expected = "Привет, как дела?";

        assert_eq!(decoded, expected, "Manual decoding failed");
    }

    #[test]
    fn test_tj_operator_decoding() {
        // Create a manual mapping
        let mut mappings = BTreeMap::new();
        mappings.insert(0x0000, vec![0x0020]); // space
        mappings.insert(0x0001, vec![0x002c]); // comma
        mappings.insert(0x0002, vec![0x003f]); // question mark
        mappings.insert(0x0003, vec![0x0432]); // Cyrillic 'в'
        mappings.insert(0x0004, vec![0x0434]); // Cyrillic 'д'
        mappings.insert(0x0005, vec![0x0438]); // Cyrillic 'и'
        mappings.insert(0x0006, vec![0x043a]); // Cyrillic 'к'
        mappings.insert(0x0007, vec![0x043b]); // Cyrillic 'л'
        mappings.insert(0x0008, vec![0x0442]); // Cyrillic 'т'
        mappings.insert(0x0009, vec![0x041f]); // Cyrillic 'П'
        mappings.insert(0x000a, vec![0x0430]); // Cyrillic 'а'
        mappings.insert(0x000b, vec![0x0435]); // Cyrillic 'е'
        mappings.insert(0x000c, vec![0x0440]); // Cyrillic 'р'

        let cmap = ToUnicodeCMap { mappings };

        // The content from the PDF's TJ operator
        let bytes = [
            0x00, 0x09, 0x00, 0x0c, 0x00, 0x05, 0x00, 0x03, 0x00, 0x0b, 0x00, 0x08, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x06, 0x00, 0x0a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x04, 0x00, 0x0b,
            0x00, 0x07, 0x00, 0x0a, 0x00, 0x02,
        ];

        // Create a TJ array
        let tj_array = vec![Object::String(bytes.to_vec(), StringFormat::Literal)];

        // Use our decode_tj_operands function
        let text_items = decode_tj_operands(&tj_array, Some(&cmap));

        // Join the text items
        let mut result = String::new();
        for item in text_items {
            if let printpdf::TextItem::Text(text) = item {
                result.push_str(&text);
            }
        }

        let expected = "Привет, как дела?";
        assert_eq!(result, expected, "TJ operator decoding failed");
    }
}
