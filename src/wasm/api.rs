#![allow(non_snake_case)]

//! Entrypoint for the WASM API. Note that this module is seperated into `mod api`
//! and `mod structs`, so that you can use the same API even on a non-WASM target,
//! without the JS limitations of having to jump through base64 encoding / decoding.

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
/// ```js,no_run,ignore
/// let html = "<!doctype html><html><body><h1>Hello!</h1></body></html>";
/// let input = JSON.encode({ html: html, title "My PDF!" });
/// let document = JSON.parse(Pdf_HtmlToPdfDocument(input));
/// console.log(document);
/// // {
/// //   status: 0,
/// //   data: {
/// //     metadata: ...,
/// //     resources: ...,
/// //     bookmarks: ...,
/// //     pages: [{ media_box, trim_box, crop_box, ops }]
/// //    }
/// // }
/// ```
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_HtmlToDocumentSync(input: String) -> String {
    api_inner(&input, crate::wasm::structs::html_to_document)
}

/// Parses the input PDF file (as a base64 encoded string), outputs the parsed
/// PDF (and any warnings) as a JSON object
///
/// ```js,no_run,ignore
/// let input = JSON.encode({ pdf_base64: atob(my_pdf) });
/// let doc = JSON.parse(Pdf_BytesToPdfDocument(input));
/// console.log(doc.pdf);
/// console.log(doc.warnings);
/// // {
/// //   status: 0,
/// //   data: {
/// //     metadata: ...,
/// //     resources: ...,
/// //     bookmarks: ...,
/// //     pages: [{ media_box, trim_box, crop_box, ops }]
/// //    }
/// // }
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
    .unwrap_or_default()
}

#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_HtmlToDocument(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::html_to_document_async(x))
    })
    .await
}

#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_BytesToDocument(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::bytes_to_document_async(x))
    })
    .await
}

#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_DocumentToBytes(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::document_to_bytes_async(x))
    })
    .await
}

#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_ResourcesForPage(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::resources_for_page_async(x))
    })
    .await
}

#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn Pdf_PageToSvg(input: String) -> String {
    api_inner_async(&input, |x| {
        Box::pin(crate::wasm::structs::page_to_svg_async(x))
    })
    .await
}

use std::{future::Future, pin::Pin};

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
            .unwrap_or_default(),
            Err(e) => serde_json::to_string(&PdfApiReturn {
                status: 2,
                data: StatusOrData::<T>::Error(e),
            })
            .unwrap_or_default(),
        },
        Err(e) => serde_json::to_string(&PdfApiReturn {
            status: 1,
            data: StatusOrData::<T>::Error(format!("failed to deserialize input: {}", e)),
        })
        .unwrap_or_default(),
    }
}
