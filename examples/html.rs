use printpdf::*;

fn main() {

    let s = r#"
        <html>
            <head>
                <style>
                    p { color: red; font-family: sans-serif; }
                </style>
            </head>
            <body>
                <p>Hello!</p>
            </body>
        </html>
    "#;

    let pages = printpdf::html::xml_to_pages(
        s, Mm(210.0), Mm(297.0)
    ).unwrap();
    
    let doc = PdfDocument::new("HTML rendering demo")
        .with_pages(pages)
        .save_to_bytes();
    
    std::fs::write("./simple.pdf", doc).unwrap();
}
