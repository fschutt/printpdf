//! HTML `<img>` -> PDF embedding example.
//!
//! Renders a small HTML document containing `<img src="cat.jpg">` to a PDF,
//! supplying the image bytes through the `images` map of
//! `PdfDocument::from_html`. The example then inspects the generated PDF to
//! prove that the JPEG was embedded as a PDF Image XObject and that the page
//! content stream references it.
//!
//! Run with:
//!     cargo run --example html_image --features "html images jpeg png"
//!
//! Output: `html_image.pdf` in the current directory.

extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;

fn main() {
    println!("Rendering HTML with <img src=\"cat.jpg\"> to PDF...");

    // The image bytes are embedded at compile time and supplied to the renderer
    // under the key "cat.jpg" — exactly the value used in the <img src> below.
    let cat_jpg: &[u8] = include_bytes!("./assets/img/cat.jpg");
    println!("Loaded cat.jpg: {} bytes", cat_jpg.len());

    // The src ("cat.jpg") becomes the lookup key. azul turns <img src="cat.jpg">
    // into an Image node tagged with "cat.jpg"; printpdf resolves that tag
    // against this map and embeds the bytes as an Image XObject.
    let html = r#"
    <html>
        <head>
            <style>
                .title { font-size: 24px; color: #222222; margin-bottom: 12px; }
                .caption { font-size: 12px; color: #666666; margin-top: 8px; }
                img { width: 300px; height: 169px; }
            </style>
        </head>
        <body>
            <div class="title">Cat photo embedded from HTML</div>
            <img src="cat.jpg" />
            <div class="caption">Rendered via &lt;img src="cat.jpg"&gt; through azul-layout + printpdf.</div>
        </body>
    </html>
    "#;

    // Supply the image bytes. Raw(...) passes bytes directly; B64(...) would pass
    // a base64 string (used by the web/WASM path). Both are decoded internally.
    let mut images: BTreeMap<String, Base64OrRaw> = BTreeMap::new();
    images.insert("cat.jpg".to_string(), Base64OrRaw::Raw(cat_jpg.to_vec()));

    let fonts: BTreeMap<String, Base64OrRaw> = BTreeMap::new();

    let options = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(20.0),
        margin_bottom: Some(20.0),
        margin_left: Some(20.0),
        ..Default::default()
    };

    let mut warnings = Vec::new();
    let doc = match PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("[ERROR] from_html failed: {}", e);
            std::process::exit(1);
        }
    };

    // ---- Evidence #1: the document carries an Image XObject for cat.jpg. ----
    let expected_id = "HtmlImg_cat_jpg"; // deterministic id derived from the src key
    let mut found_xobject = false;
    let mut xobject_dims = None;
    for (id, xobj) in doc.resources.xobjects.map.iter() {
        if let XObject::Image(raw) = xobj {
            println!(
                "[OK] Image XObject in resources: id={:?} {}x{}px",
                id, raw.width, raw.height
            );
            if id.0 == expected_id {
                found_xobject = true;
                xobject_dims = Some((raw.width, raw.height));
            }
        }
    }
    assert!(
        found_xobject,
        "expected an Image XObject registered under id {expected_id:?}"
    );

    // ---- Evidence #2: a page content stream references the XObject via UseXobject. ----
    let mut found_use_xobject = false;
    for page in doc.pages.iter() {
        for op in page.ops.iter() {
            if let Op::UseXobject { id, transform } = op {
                if id.0 == expected_id {
                    found_use_xobject = true;
                    println!(
                        "[OK] Page references image: Op::UseXobject id={:?} scale=({:?},{:?}) translate=({:?},{:?})",
                        id, transform.scale_x, transform.scale_y, transform.translate_x, transform.translate_y
                    );
                }
            }
        }
    }
    assert!(
        found_use_xobject,
        "expected a Op::UseXobject referencing {expected_id:?} in a page content stream"
    );

    // ---- Evidence #3: the serialized PDF actually contains an Image XObject. ----
    let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write("html_image.pdf", &pdf_bytes).expect("write html_image.pdf");
    println!("Wrote html_image.pdf ({} bytes)", pdf_bytes.len());

    // lopdf renders the dict without spaces (e.g. `/Subtype/Image`); accept both.
    let contains = |needle: &[u8]| pdf_bytes.windows(needle.len()).any(|w| w == needle);
    let contains_image_xobject = contains(b"/Subtype/Image") || contains(b"/Subtype /Image");
    assert!(
        contains_image_xobject,
        "serialized PDF does not contain an Image XObject (/Subtype/Image)"
    );
    println!("[OK] Serialized PDF contains an Image XObject (/Subtype/Image)");

    // The cat.jpg round-trips as a JPEG (DCTDecode) stream.
    let has_filter = contains(b"/Filter/DCTDecode") || contains(b"/Filter /DCTDecode");
    assert!(has_filter, "image XObject should be a DCTDecode (JPEG) stream");
    println!("[OK] Image XObject is a DCTDecode (JPEG) stream");

    // The image dictionary declares /Width and /Height (the serializer may
    // down-scale to fit a size budget, so we don't assert the exact values).
    let _ = xobject_dims;
    assert!(
        contains(b"/Width") && contains(b"/Height"),
        "serialized PDF must declare image /Width and /Height"
    );
    println!("[OK] PDF image dictionary declares /Width and /Height");

    println!("\nSUCCESS: <img src=\"cat.jpg\"> was embedded as a PDF Image XObject.");
}
