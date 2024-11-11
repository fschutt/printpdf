use printpdf::*;

fn main() {
    let mut doc = PdfDocument::new("My first PDF");
    let image_bytes = include_bytes!("assets/img/dog_alpha.png");
    let image = RawImage::decode_from_bytes(image_bytes).unwrap(); // requires --feature bmp

    // In the PDF, an image is an `XObject`, identified by a unique `ImageId`
    let image_xobject_id = doc.add_image(&image);

    let page1_contents = vec![Op::UseXObject {
        id: image_xobject_id.clone(),
        transform: XObjectTransform::default(),
    }];

    let page1 = PdfPage::new(Mm(210.0), Mm(297.0), page1_contents);
    let pdf_bytes: Vec<u8> = doc.with_pages(vec![page1]).save(&PdfSaveOptions::default());
    let _ = std::fs::write("image.pdf", pdf_bytes);
}
