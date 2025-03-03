//! Datastructures for the WASM API entrypoint. Useful if you want to
//! use the same WASM API but without JS serialization / deserialization

use std::collections::BTreeMap;

use base64::Engine;
use serde_derive::{Deserialize, Serialize};

use crate::{
    FontId, ImageOptimizationOptions, LayerInternalId, Mm, PdfDocument, PdfPage, PdfParseOptions,
    PdfResources, PdfSaveOptions, PdfToSvgOptions, PdfWarnMsg, XObjectId,
};

/// Base64 is necessary because there are a lot of JS issues surrounding
/// `ArrayBuffer` / `Uint8Buffer` / `ByteArray` type mismatches, so a simple
/// `atob` / `btoa` fixes that at the cost of slight performance decrease.
///
/// Note: this enum is untagged, so from JS you can pass in either the base64 bytes
/// or the bytearray and it'll work.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Base64OrRaw {
    /// Base64 string, usually tagged with
    B64(String),
    /// Raw bytes
    Raw(Vec<u8>),
}

impl Default for Base64OrRaw {
    fn default() -> Self {
        Base64OrRaw::Raw(Vec::new())
    }
}

impl Base64OrRaw {
    // Decodes the bytes if base64 and also gets rid of the "data:...;base64," prefix
    pub fn decode_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            Base64OrRaw::B64(r) => base64::prelude::BASE64_STANDARD
                .decode(get_base64_substr(r))
                .map_err(|e| e.to_string()),
            Base64OrRaw::Raw(r) => Ok(r.clone()),
        }
    }
}

fn get_base64_substr(input: &str) -> &str {
    // Check if the input starts with "data:" and contains a comma.
    if input.starts_with("data:") {
        if let Some(comma_index) = input.find(',') {
            // Optionally, verify that the metadata contains "base64"
            let metadata = &input[..comma_index];
            if metadata.contains("base64") {
                // Return the portion after the comma
                &input[comma_index + 1..]
            } else {
                // If not marked as base64, assume the whole string is encoded
                input
            }
        } else {
            // No comma found; fall back to using the entire string
            input
        }
    } else {
        // Not a data URL, so use the string as-is
        input
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct HtmlToDocumentInput {
    /// Input HTML to generate the pages from, required parameter
    pub html: String,

    /// Title of the PDF document, optional = empty
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GeneratePdfOptions {
    /// Whether to embed fonts in the PDF (default: true)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_embedding: Option<bool>,
    /// Page width in mm, default 210.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_width: Option<f32>,
    /// Page height in mm, default 297.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_height: Option<f32>,
    /// Settings for automatic image optimization when saving PDF files
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_optimization: Option<ImageOptimizationOptions>,
}

impl Default for GeneratePdfOptions {
    fn default() -> Self {
        Self {
            font_embedding: Some(true),
            page_width: Some(210.0),
            page_height: Some(297.0),
            image_optimization: None,
        }
    }
}

impl GeneratePdfOptions {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(feature = "html")]
pub fn html_to_document(input: HtmlToDocumentInput) -> Result<HtmlToDocumentOutput, String> {
    let mut warnings = Vec::new();
    let (transformed_xml, mut pdf, opts) = html_to_document_inner(input)?;
    let pages = pdf.html_to_pages(&transformed_xml, opts, &mut warnings)?;
    pdf.with_pages(pages);
    Ok(HtmlToDocumentOutput { doc: pdf, warnings })
}

#[cfg(feature = "html")]
pub async fn html_to_document_async(
    input: HtmlToDocumentInput,
) -> Result<HtmlToDocumentOutput, String> {
    let mut warnings = Vec::new();
    let (transformed_xml, mut pdf, opts) = html_to_document_inner(input)?;
    let pages = pdf
        .html_to_pages_async(&transformed_xml, opts, &mut warnings)
        .await?;
    pdf.with_pages(pages);
    Ok(HtmlToDocumentOutput { doc: pdf, warnings })
}

#[cfg(feature = "html")]
fn html_to_document_inner(
    input: HtmlToDocumentInput,
) -> Result<(String, PdfDocument, crate::XmlRenderOptions), String> {
    // Transform HTML to XML with extracted configuration
    let (transformed_xml, config) = crate::html::process_html_for_rendering(&input.html);

    // Create document with title from input or extracted from HTML
    let title = if input.title.is_empty() {
        config.title.clone().unwrap_or_default()
    } else {
        input.title.clone()
    };

    let mut pdf = crate::PdfDocument::new(&title);

    // Prepare rendering options
    let mut opts = crate::html::XmlRenderOptions {
        page_width: Mm(input.options.page_width.unwrap_or(210.0)),
        page_height: Mm(input.options.page_height.unwrap_or(297.0)),
        images: input
            .images
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.decode_bytes().ok()?)))
            .collect(),
        fonts: input
            .fonts
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.decode_bytes().ok()?)))
            .collect(),
        components: Vec::new(),
    };

    // Apply configuration from HTML to document and options
    crate::html::apply_html_config(&mut pdf, &config, &mut opts);

    // Register component nodes extracted from HTML
    for component_node in config.components {
        opts.components.push(azul_core::xml::XmlComponent {
            id: component_node.name.clone(),
            renderer: Box::new(component_node),
            inherit_vars: false,
        });
    }

    Ok((transformed_xml, pdf, opts))
}

const ERR: &str = "Pdf_HtmlToDocument failed: feature --html not enabled for printpdf crate";

#[cfg(not(feature = "html"))]
pub fn html_to_document(input: HtmlToDocumentInput) -> Result<HtmlToDocumentOutput, String> {
    Err(ERR.to_string())
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
