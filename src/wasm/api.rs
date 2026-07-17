#![allow(non_snake_case)]

//! Entrypoint for the WASM API. Note that this module is seperated into `mod api`
//! and `mod structs`, so that you can use the same API even on a non-WASM target,
//! without the JS limitations of having to jump through base64 encoding / decoding.

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
use std::{future::Future, pin::Pin};

use serde_derive::{Deserialize, Serialize};

/// Generalized API return for WASM / JS
#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub struct PdfApiReturn<T: serde::Serialize> {
    /// If "status" is 0, then data contains the processed data
    /// If non-zero, data is the error string.
    pub status: usize,
    /// Data or error of the function called
    pub data: StatusOrData<T>,
}

/// Data or error of the output of the function.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum StatusOrData<T: serde::Serialize> {
    Ok(T),
    Error(String),
}

/// Parses the input HTML, converts it to PDF pages and outputs the generated
/// PDF as a JSON object
///
/// Input: `{ html, images?, fonts?, options? }` (see
/// [`crate::wasm::structs::HtmlToDocumentInput`]; `options` fields are
/// snake_case, e.g. `page_width`).
///
/// ```js,no_run,ignore
/// let html = "<!doctype html><html><body><h1>Hello!</h1></body></html>";
/// let input = JSON.stringify({ html: html, fonts: {}, images: {} });
/// let result = JSON.parse(Pdf_HtmlToDocumentSync(input));
/// console.log(result);
/// // {
/// //   status: 0,   // 1 = bad input, 2 = conversion failed, 3 = output unserializable
/// //   data: {
/// //     doc: {
/// //       metadata: { info, xmp? },
/// //       resources: { fonts, xobjects, extgstates, shadings, layers },
/// //       bookmarks: {},
/// //       pages: [{ mediaBox, trimBox, cropBox, ops }]
/// //     },
/// //     warnings: [{ page, opId, severity, msg }]
/// //   }   // on status != 0, `data` is the error string instead
/// // }
/// ```
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_HtmlToDocumentSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::html_to_document)
}

/// Parses the input PDF file (as a base64 encoded string or raw byte array),
/// outputs the parsed PDF (and any warnings) as a JSON object
///
/// ```js,no_run,ignore
/// let input = JSON.stringify({ bytes: btoa(my_pdf_binary_string), options: {} });
/// let result = JSON.parse(Pdf_BytesToDocumentSync(input));
/// console.log(result.data.doc);      // PdfDocument (same shape as Pdf_HtmlToDocumentSync)
/// console.log(result.data.warnings); // [{ page, opId, severity, msg }]
/// ```
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_BytesToDocumentSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::bytes_to_document)
}

/// Helper function that takes a PDF page and outputs a list of all
/// images IDs / fonts IDs that have to be gathered from the documents
/// resources in order to render this page.
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_ResourcesForPageSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::resources_for_page)
}

/// Takes a `PdfPage` JS object and outputs the SVG string for that page
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_PageToSvgSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::page_to_svg)
}

/// Takes a `PdfDocument` JS object and returns the base64 PDF bytes
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_DocumentToBytesSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::document_to_bytes)
}

fn api_inner<'a, T, Q>(input: &'a str, f: fn(Q) -> Result<T, String>) -> String
where
    T: serde::Serialize,
    Q: serde::Deserialize<'a>,
{
    serde_json::to_string(&match serde_json::from_str::<Q>(input) {
        Ok(input) => match (f)(input) {
            Ok(o) => PdfApiReturn {
                status: 0,
                data: StatusOrData::Ok(o),
            },
            Err(e) => PdfApiReturn {
                status: 2,
                data: StatusOrData::Error(e),
            },
        },
        Err(e) => PdfApiReturn {
            status: 1,
            data: StatusOrData::Error(format!("failed to deserialize input: {e}")),
        },
    })
    .unwrap_or_else(|e| output_serialization_error_envelope(&e))
}

/// Fallback envelope for when the *response* cannot be serialized (e.g. a
/// `ParsedFont` in the output document errors in its custom `Serialize`).
///
/// This used to be `unwrap_or_default()`, i.e. the API returned an **empty
/// string** — which every caller (script.js does `JSON.parse(result)`
/// unconditionally) turned into an unrelated "Unexpected end of JSON input"
/// error with the actual cause lost. Always hand out a well-formed
/// `{"status":3,"data":"<why>"}` envelope instead.
fn output_serialization_error_envelope(e: &serde_json::Error) -> String {
    // Serializing a `&str` cannot fail, but avoid any chance of recursing.
    let msg = serde_json::to_string(&format!("failed to serialize output: {e}"))
        .unwrap_or_else(|_| "\"failed to serialize output\"".to_string());
    format!("{{\"status\":3,\"data\":{msg}}}")
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_HtmlToDocument(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::html_to_document_async(x))
    })
    .await
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_BytesToDocument(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::bytes_to_document_async(x))
    })
    .await
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_DocumentToBytes(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::document_to_bytes_async(x))
    })
    .await
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_ResourcesForPage(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::resources_for_page_async(x))
    })
    .await
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_PageToSvg(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::page_to_svg_async(x))
    })
    .await
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
async fn api_inner_async<T, Q>(
    input: &str,
    f: fn(Q) -> Pin<Box<dyn Future<Output = Result<T, String>>>>,
) -> String
where
    T: serde::Serialize,
    Q: for<'de> serde::Deserialize<'de>,
{
    match serde_json::from_str::<Q>(input) {
        Ok(input_obj) => match f(input_obj).await {
            Ok(data) => serde_json::to_string(&PdfApiReturn {
                status: 0,
                data: StatusOrData::Ok(data),
            })
            .unwrap_or_else(|e| output_serialization_error_envelope(&e)),
            Err(e) => serde_json::to_string(&PdfApiReturn {
                status: 2,
                data: StatusOrData::<T>::Error(e),
            })
            .unwrap_or_else(|e| output_serialization_error_envelope(&e)),
        },
        Err(e) => serde_json::to_string(&PdfApiReturn {
            status: 1,
            data: StatusOrData::<T>::Error(format!("failed to deserialize input: {}", e)),
        })
        .unwrap_or_else(|e| output_serialization_error_envelope(&e)),
    }
}

/// Registers fonts once for ALL subsequent `Pdf_HtmlToDocument` calls (merged
/// under each call's own `fonts`, which win on name clash). Call this at app
/// startup with your default fonts instead of re-sending megabytes of base64
/// with every render.
///
/// ```js,no_run,ignore
/// let r = JSON.parse(Pdf_RegisterFontsSync(JSON.stringify({
///   fonts: { "Helvetica.ttf": HELVETICA_B64 },
/// })));
/// // r == { status: 0, data: { registered: 1, total: 1 } }
/// ```
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_RegisterFontsSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::register_fonts)
}

/// Decodes an image file (PNG/JPEG/...) into the `RawImage` JSON shape used in
/// `doc.resources.xobjects`, so JS can add images/signatures to a parsed
/// document and reference them with a `use-xobject` op.
///
/// ```js,no_run,ignore
/// let r = JSON.parse(Pdf_DecodeImageSync(JSON.stringify({ bytes: pngB64 })));
/// // r == { status: 0, data: { image: { width, height, ... }, warnings: [] } }
/// ```
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_DecodeImageSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::decode_image)
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_RegisterFonts(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::register_fonts_async(x))
    })
    .await
}

#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_DecodeImage(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::decode_image_async(x))
    })
    .await
}

/// Forward Rust panic messages to the browser console. Without this hook a
/// panic surfaces as an opaque `RuntimeError: unreachable` with no message at
/// all — exactly how the demo's first-render failure presented during the 0.12
/// rework debugging.
#[cfg(all(target_family = "wasm", feature = "js-sys"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn printpdf_wasm_init() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("printpdf wasm panic: {info}").into());
    }));
}
