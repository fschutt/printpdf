use printpdf::*;

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Image Example");

    // Load an image
    let image_bytes = include_bytes!("./assets/img/dog_alpha.png");
    let image = RawImage::decode_from_bytes(image_bytes, &mut Vec::new()).unwrap();

    // Create operations for our page
    let mut ops = Vec::new();

    // Add the image to the document resources and get its ID
    let image_id = doc.add_image(&image);

    // Place the image with default transform (at 0,0)
    ops.push(Op::UseXobject {
        id: image_id.clone(),
        transform: XObjectTransform::default(),
    });

    // Place the same image again, but translated, rotated, and scaled
    ops.push(Op::UseXobject {
        id: image_id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(300.0)),
            translate_y: Some(Pt(300.0)),
            rotate: Some(XObjectRotation {
                angle_ccw_degrees: 45.0,
                rotation_center_x: Px(100),
                rotation_center_y: Px(100),
            }),
            scale_x: Some(0.5),
            scale_y: Some(0.5),
            dpi: Some(300.0),
        },
    });

    // Create a page with our operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

    // Save the PDF to a file
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    
    std::fs::write("./image_example.pdf", bytes).unwrap();
    println!("Created image_example.pdf");
}