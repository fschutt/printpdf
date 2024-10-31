use serde_derive::{Serialize, Deserialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PrintPdfApiInput {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub html: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, Vec<u8>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fonts: BTreeMap<String, Vec<u8>>,
    #[serde(default, skip_serializing_if = "PdfGenerationOptions::is_default")]
    pub options: PdfGenerationOptions,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct PdfGenerationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dont_compress_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_entire_fonts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_width_mm: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_height_mm: Option<f32>,
}

impl PdfGenerationOptions {
    fn is_default(&self) -> bool { *self == Self::default() }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub struct PrintPdfApiReturn {
    pub status: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pdf: Vec<u8>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

#[allow(non_snake_case)]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn PrintPdfFromXml(input: String) -> String {
    let init = match serde_json::from_str::<PrintPdfApiInput>(&input) {
        Ok(o) => match printpdf_from_xml_internal(o) {
            Ok(o) => o,
            Err(e) => e,
        },
        Err(e) => PrintPdfApiReturn {
            pdf: Vec::new(),
            status: 1,
            error: format!("failed to parse input parameters: {}", e.to_string()),
        }
    };
    serde_json::to_string(&init).unwrap_or_default()
}

fn printpdf_from_xml_internal(input: PrintPdfApiInput) -> Result<PrintPdfApiReturn, PrintPdfApiReturn> {

    use crate::units::Mm;

    // TODO: extract document title from XML!

    let pages = crate::html::xml_to_pages(
        &input.html, 
        Mm(input.options.page_width_mm.unwrap_or(210.0)), 
        Mm(input.options.page_width_mm.unwrap_or(297.0))
    ).map_err(|e| PrintPdfApiReturn {
        pdf: Vec::new(),
        status: 2,
        error: e,
    })?;
    
    let pdf = crate::PdfDocument::new("PDF")
        .with_pages(pages)
        .save_to_bytes();
    
    Ok(PrintPdfApiReturn { pdf, status: 0, error: String::new() })
}
