//! Datastructures for the WASM API entrypoint. Useful if you want to
//! use the same WASM API but without JS serialization / deserialization

use std::collections::BTreeMap;

use base64::Engine;
use serde_derive::{Deserialize, Serialize};

use crate::{
    Base64OrRaw, FontId, GeneratePdfOptions, LayerInternalId, PdfDocument, PdfPage,
    PdfParseOptions, PdfResources, PdfSaveOptions, PdfToSvgOptions, PdfWarnMsg, XObjectId,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct HtmlToDocumentInput {
    /// Input HTML to generate the pages from, required parameter
    pub html: String,
    /// Input images (i.e. "dog.png" => Base64String)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, Base64OrRaw>,
    /// Input fonts (i.e. "Roboto.ttf" => Base64String)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fonts: BTreeMap<String, Base64OrRaw>,
    /// Miscellaneous options
    #[serde(default, skip_serializing_if = "GeneratePdfOptions::is_default")]
    pub options: GeneratePdfOptions,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct HtmlToDocumentOutput {
    /// Generated PDF document
    pub doc: PdfDocument,
    #[serde(default)]
    pub warnings: Vec<PdfWarnMsg>,
}

#[cfg(feature = "html")]
pub fn html_to_document(input: HtmlToDocumentInput) -> Result<HtmlToDocumentOutput, String> {
    let mut warnings = Vec::new();
    let pdf = PdfDocument::from_html(
        &input.html,
        &input.images,
        &input.fonts,
        &input.options,
        &mut warnings,
    )?;
    Ok(HtmlToDocumentOutput { doc: pdf, warnings })
}

#[cfg(not(feature = "html"))]
const ERR: &str = "Pdf_HtmlToDocument failed: feature --html not enabled for printpdf crate";

#[cfg(not(feature = "html"))]
pub fn html_to_document(input: HtmlToDocumentInput) -> Result<HtmlToDocumentOutput, String> {
    Err(ERR.to_string())
}

#[cfg(feature = "html")]
pub async fn html_to_document_async(
    input: HtmlToDocumentInput,
) -> Result<HtmlToDocumentOutput, String> {
    let mut warnings = Vec::new();
    let pdf = PdfDocument::from_html(
        &input.html,
        &input.images,
        &input.fonts,
        &input.options,
        &mut warnings,
    )?;
    Ok(HtmlToDocumentOutput { doc: pdf, warnings })
}

#[cfg(not(feature = "html"))]
pub async fn html_to_document_async(
    input: HtmlToDocumentInput,
) -> Result<HtmlToDocumentOutput, String> {
    Err(ERR.to_string())
}

// Vec<u8> -> PdfDocument

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BytesToDocumentInput {
    pub bytes: Base64OrRaw,
    #[serde(default)]
    pub options: PdfParseOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BytesToDocumentOutput {
    pub doc: PdfDocument,
    #[serde(default)]
    pub warnings: Vec<PdfWarnMsg>,
}

pub fn bytes_to_document(input: BytesToDocumentInput) -> Result<BytesToDocumentOutput, String> {
    let bytes = input
        .bytes
        .decode_bytes()
        .map_err(|e| format!("failed to decode input bytes: {e}"))?;

    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(&bytes, &input.options, &mut warnings)
        .map_err(|e| format!("failed to parse PDF: {e}"))?;

    Ok(BytesToDocumentOutput { doc, warnings })
}

pub async fn bytes_to_document_async(
    input: BytesToDocumentInput,
) -> Result<BytesToDocumentOutput, String> {
    let bytes = input
        .bytes
        .decode_bytes()
        .map_err(|e| format!("failed to decode input bytes: {e}"))?;

    let mut warnings = Vec::new();
    let doc = PdfDocument::parse_async(&bytes, &input.options, &mut warnings)
        .await
        .map_err(|e| format!("failed to parse PDF: {e}"))?;

    Ok(BytesToDocumentOutput { doc, warnings })
}

// PdfDocument -> Vec<u8>

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentToBytesInput {
    /// Required: the document to encode to PDF bytes
    pub doc: PdfDocument,
    /// Optional: Options on how to save the PDF file, image compression, etc.
    #[serde(default)]
    pub options: PdfSaveOptions,
    /// Optional: Whether to return the raw bytes instead of a base64 string. Default: false
    #[serde(default)]
    pub return_byte_array: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentToBytesOutput {
    /// Either base64 bytes or raw [u8], depending on
    pub bytes: Base64OrRaw,
    pub warnings: Vec<PdfWarnMsg>,
}

pub fn document_to_bytes(input: DocumentToBytesInput) -> Result<DocumentToBytesOutput, String> {
    let mut warnings = Vec::new();
    let return_byte_array = input.return_byte_array;
    let bytes = input.doc.save(&input.options, &mut warnings);

    Ok(DocumentToBytesOutput {
        bytes: if return_byte_array {
            Base64OrRaw::Raw(bytes)
        } else {
            Base64OrRaw::B64(base64::prelude::BASE64_STANDARD.encode(&bytes))
        },
        warnings,
    })
}

pub async fn document_to_bytes_async(
    input: DocumentToBytesInput,
) -> Result<DocumentToBytesOutput, String> {
    let mut warnings = Vec::new();
    let return_byte_array = input.return_byte_array;
    let bytes = input.doc.save_async(&input.options, &mut warnings).await;

    Ok(DocumentToBytesOutput {
        bytes: if return_byte_array {
            Base64OrRaw::Raw(bytes)
        } else {
            Base64OrRaw::B64(base64::prelude::BASE64_STANDARD.encode(&bytes))
        },
        warnings,
    })
}

// PdfPage -> PdfResourceIds

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesForPageInput {
    /// Required: the PDF page to get the resources from.
    pub page: PdfPage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesForPageOutput {
    /// Images, forms, external content
    pub xobjects: Vec<XObjectId>,
    /// External fonts used on this page
    pub fonts: Vec<FontId>,
    /// Layers, including info on this page
    pub layers: Vec<LayerInternalId>,
}

pub fn resources_for_page(input: ResourcesForPageInput) -> Result<ResourcesForPageOutput, String> {
    Ok(ResourcesForPageOutput {
        xobjects: input.page.get_xobject_ids(),
        fonts: input.page.get_external_font_ids(),
        layers: input.page.get_layers(),
    })
}

pub async fn resources_for_page_async(
    input: ResourcesForPageInput,
) -> Result<ResourcesForPageOutput, String> {
    resources_for_page(input)
}

// PdfPage -> SvgString

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageToSvgInput {
    pub page: PdfPage,
    #[serde(default)]
    pub resources: PdfResources,
    #[serde(default)]
    pub options: PdfToSvgOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageToSvgOutput {
    /// The generated SVG string for the page.
    pub svg: String,
    /// Warnings generated during rendering
    pub warnings: Vec<PdfWarnMsg>,
}

pub fn page_to_svg(input: PageToSvgInput) -> Result<PageToSvgOutput, String> {
    let mut warnings = Vec::new();
    let svg =
        crate::render::render_to_svg(&input.page, &input.resources, &input.options, &mut warnings);
    Ok(PageToSvgOutput { svg, warnings })
}

pub async fn page_to_svg_async(input: PageToSvgInput) -> Result<PageToSvgOutput, String> {
    let mut warnings = Vec::new();
    let svg = crate::render::render_to_svg_async(
        &input.page,
        &input.resources,
        &input.options,
        &mut warnings,
    )
    .await;
    Ok(PageToSvgOutput { svg, warnings })
}
