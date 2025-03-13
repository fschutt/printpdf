fn main() {
    let bytes = include_bytes!("/Users/fschutt/Downloads/2303.12712v5.pdf");
    let time1 = std::time::Instant::now();
    let parsed = printpdf::PdfDocument::parse(
        bytes,
        &printpdf::PdfParseOptions::default(),
        &mut Vec::new(),
    )
    .unwrap();
    let mut s = String::new();
    for p in parsed.pages.iter() {
        s.push_str(&p.extract_text(&parsed.resources).join("\r\n"));
    }
    println!("string: {}", s.len());
    let time2 = std::time::Instant::now();
    std::fs::write("./test.txt", &s);
    println!("time: {:?}", time2 - time1);
}
