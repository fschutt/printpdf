use std::collections::BTreeMap;

use base64::prelude::*;
use serde_derive::{Deserialize, Serialize};

use crate::{
    FontId, LayerInternalId, PdfDocument, PdfPage, PdfParseOptions, PdfResources, PdfSaveOptions,
    PdfToSvgOptions, PdfWarnMsg, XObjectId, XmlRenderOptions, units::Mm,
};

pub type Base64String = String;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct PrintPdfHtmlInput {
    /// Title of the PDF document
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Input HTML
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub html: String,
    /// Input images (i.e. "dog.png" => Base64String)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, Base64String>,
    /// Input fonts (i.e. "Roboto.ttf" => Base64String)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fonts: BTreeMap<String, Base64String>,
    /// Miscellaneous options
    #[serde(default, skip_serializing_if = "PdfGenerationOptions::is_default")]
    pub options: PdfGenerationOptions,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PdfGenerationOptions {
    /// Whether to compress images and if yes, to what quality level
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_compression: Option<f32>,
    /// Whether to embed fonts in the PDF (default: true)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_embedding: Option<bool>,
    /// Page width in mm, default 210.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_width: Option<f32>,
    /// Page height in mm, default 297.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_height: Option<f32>,
}

impl Default for PdfGenerationOptions {
    fn default() -> Self {
        Self {
            image_compression: None,
            font_embedding: Some(true),
            page_width: Some(210.0),
            page_height: Some(297.0),
        }
    }
}

impl PdfGenerationOptions {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub struct PrintPdfApiReturn<T: serde::Serialize> {
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
pub fn Pdf_HtmlToPdfDocument(input: String) -> String {
    let input = match serde_json::from_str::<PrintPdfHtmlInput>(&input) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 1,
                data: StatusOrData::<PdfDocument>::Error(format!(
                    "failed to parse input parameters: {e}"
                )),
            })
            .unwrap_or_default();
        }
    };

    let document = match pdf_html_to_json(input) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 2,
                data: StatusOrData::<PdfDocument>::Error(e),
            })
            .unwrap_or_default();
        }
    };

    serde_json::to_string(&PrintPdfApiReturn {
        status: 0,
        data: StatusOrData::Ok(document),
    })
    .unwrap_or_default()
}

fn pdf_html_to_json(input: PrintPdfHtmlInput) -> Result<PdfDocument, String> {
    // TODO: extract document title from XML!
    let opts = XmlRenderOptions {
        page_width: Mm(input.options.page_width.unwrap_or(210.0)),
        page_height: Mm(input.options.page_height.unwrap_or(297.0)),
        images: input
            .images
            .iter()
            .filter_map(|(k, v)| {
                Some((k.clone(), base64::prelude::BASE64_STANDARD.decode(v).ok()?))
            })
            .collect(),
        fonts: input
            .fonts
            .iter()
            .filter_map(|(k, v)| {
                Some((k.clone(), base64::prelude::BASE64_STANDARD.decode(v).ok()?))
            })
            .collect(),
        components: Vec::new(),
    };

    let mut pdf = crate::PdfDocument::new(&input.title);

    let pages = pdf.html2pages(&input.html, opts)?;

    pdf.with_pages(pages);

    Ok(pdf)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfParseInput {
    pub pdf_base64: String,
    #[serde(default)]
    pub options: PdfParseOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfParseOutput {
    pub pdf: PdfDocument,
    #[serde(default)]
    pub warnings: Vec<PdfWarnMsg>,
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
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_BytesToPdfDocument(input: String) -> String {
    let input = match serde_json::from_str::<PrintPdfParseInput>(&input) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 1,
                data: StatusOrData::<PdfDocument>::Error(format!(
                    "failed to parse input parameters: {e}"
                )),
            })
            .unwrap_or_default();
        }
    };

    let bytes = match base64::prelude::BASE64_STANDARD.decode(&input.pdf_base64) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 2,
                data: StatusOrData::<PdfDocument>::Error(format!(
                    "failed to parse decode input.pdf_base64 as base64: {e}"
                )),
            })
            .unwrap_or_default();
        }
    };

    let (doc, warn) = match PdfDocument::parse(&bytes, &input.options) {
        Ok((doc, warn)) => (doc, warn),
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 3,
                data: StatusOrData::<PdfDocument>::Error(format!("failed to parse PDF: {e}")),
            })
            .unwrap_or_default();
        }
    };

    let output = PrintPdfApiReturn {
        status: 0,
        data: StatusOrData::Ok(PrintPdfParseOutput {
            pdf: doc,
            warnings: warn,
        }),
    };

    serde_json::to_string(&output).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfPageGetResourcesInput {
    pub page: PdfPage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfPageGetResourcesOutput {
    /// Images, forms, external content
    pub xobjects: Vec<XObjectId>,
    /// External fonts used on this page
    pub fonts: Vec<FontId>,
    /// Layers, including info on this page
    pub layers: Vec<LayerInternalId>,
}

/// Helper function that takes a PDF page and outputs a list of all
/// images IDs / fonts IDs that have to be gathered from the documents
/// resources in order to render this page.
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_GetResourcesForPage(input: String) -> String {
    let input = match serde_json::from_str::<PrintPdfPageGetResourcesInput>(&input) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 1,
                data: StatusOrData::<PrintPdfPageToSvgOutput>::Error(format!(
                    "failed to parse input parameters: {e}"
                )),
            })
            .unwrap_or_default();
        }
    };

    let output = PrintPdfPageGetResourcesOutput {
        xobjects: input.page.get_xobject_ids(),
        fonts: input.page.get_external_font_ids(),
        layers: input.page.get_layers(),
    };

    serde_json::to_string(&PrintPdfApiReturn {
        status: 0,
        data: StatusOrData::Ok(output),
    })
    .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfPageToSvgInput {
    pub page: PdfPage,
    #[serde(default)]
    pub resources: PdfResources,
    #[serde(default)]
    pub options: PdfToSvgOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfPageToSvgOutput {
    pub svg: String,
}

/// Takes a `PdfPage` JS object and outputs the SVG string for that page
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_PdfPageToSvg(input: String) -> String {
    let input = match serde_json::from_str::<PrintPdfPageToSvgInput>(&input) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 1,
                data: StatusOrData::<PrintPdfPageToSvgOutput>::Error(format!(
                    "failed to parse input parameters: {e}"
                )),
            })
            .unwrap_or_default();
        }
    };

    let svg = input.page.to_svg(&input.resources, &input.options);

    serde_json::to_string(&PrintPdfApiReturn {
        status: 0,
        data: StatusOrData::Ok(PrintPdfPageToSvgOutput { svg }),
    })
    .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfToBytesInput {
    pub pdf: PdfDocument,
    pub options: PdfSaveOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPdfToBytesOutput {
    pub pdf_base64: String,
}

/// Takes a `PdfDocument` JS object and returns the base64 PDF bytes
#[allow(non_snake_case)]
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn Pdf_PdfDocumentToBytes(input: String) -> String {
    let input = match serde_json::from_str::<PrintPdfToBytesInput>(&input) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&PrintPdfApiReturn {
                status: 1,
                data: StatusOrData::<PdfDocument>::Error(format!(
                    "failed to parse input parameters: {e}"
                )),
            })
            .unwrap_or_default();
        }
    };

    let bytes = base64::prelude::BASE64_STANDARD.encode(input.pdf.save(&input.options));

    serde_json::to_string(&PrintPdfApiReturn {
        status: 0,
        data: StatusOrData::Ok(PrintPdfToBytesOutput { pdf_base64: bytes }),
    })
    .unwrap_or_default()
}
