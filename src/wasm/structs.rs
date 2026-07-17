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
    // Fonts registered via `Pdf_RegisterFonts` participate in every render.
    let fonts = fonts_with_registered(&input.fonts);
    let pdf = PdfDocument::from_html(
        &input.html,
        &input.images,
        &fonts,
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
    // Fonts registered via `Pdf_RegisterFonts` participate in every render.
    let fonts = fonts_with_registered(&input.fonts);
    let pdf = PdfDocument::from_html(
        &input.html,
        &input.images,
        &fonts,
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

// --- Font registry: register once, use in every subsequent render -----------

/// Fonts registered once via `Pdf_RegisterFonts` and merged under every
/// `html_to_document` call's own fonts (per-call entries win on name clash).
///
/// The demo page carries ~10 MB of default fonts; before this registry existed,
/// script.js re-sent all of them inside the input JSON on every re-render —
/// i.e. on every keystroke.
static REGISTERED_FONTS: std::sync::Mutex<BTreeMap<String, Vec<u8>>> =
    std::sync::Mutex::new(BTreeMap::new());

/// Registry fonts (as `Raw`) overlaid with the call's own fonts.
pub(crate) fn fonts_with_registered(
    input_fonts: &BTreeMap<String, Base64OrRaw>,
) -> BTreeMap<String, Base64OrRaw> {
    let mut merged: BTreeMap<String, Base64OrRaw> = REGISTERED_FONTS
        .lock()
        .map(|reg| {
            reg.iter()
                .map(|(k, v)| (k.clone(), Base64OrRaw::Raw(v.clone())))
                .collect()
        })
        .unwrap_or_default();
    for (k, v) in input_fonts {
        merged.insert(k.clone(), v.clone());
    }
    merged
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct RegisterFontsInput {
    /// Fonts to keep registered ("Roboto.ttf" => Base64String or raw bytes).
    #[serde(default)]
    pub fonts: BTreeMap<String, Base64OrRaw>,
    /// Clear all previously registered fonts first.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RegisterFontsOutput {
    /// Number of fonts (de)registered by this call
    pub registered: usize,
    /// Total number of fonts now in the registry
    pub total: usize,
}

pub fn register_fonts(input: RegisterFontsInput) -> Result<RegisterFontsOutput, String> {
    let mut reg = REGISTERED_FONTS
        .lock()
        .map_err(|_| "font registry lock poisoned".to_string())?;
    if input.replace {
        reg.clear();
    }
    let mut registered = 0;
    for (name, b) in &input.fonts {
        let bytes = b
            .decode_bytes()
            .map_err(|e| format!("font {name:?}: {e}"))?;
        reg.insert(name.clone(), bytes);
        registered += 1;
    }
    Ok(RegisterFontsOutput {
        registered,
        total: reg.len(),
    })
}

pub async fn register_fonts_async(
    input: RegisterFontsInput,
) -> Result<RegisterFontsOutput, String> {
    register_fonts(input)
}

// --- Image decoding (for the sign-pdf / add-image flows) --------------------

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DecodeImageInput {
    /// Encoded image file (PNG/JPEG/... — whatever image-format features are enabled)
    pub bytes: Base64OrRaw,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DecodeImageOutput {
    /// The decoded image as the `RawImage` JSON the document model uses in
    /// `resources.xobjects` — insert it there and reference it with a
    /// `use-xobject` op.
    pub image: crate::RawImage,
    #[serde(default)]
    pub warnings: Vec<PdfWarnMsg>,
}

/// Decode an image file into the `RawImage` JSON shape. This closes the API gap
/// that made the demo's sign-pdf tab impossible: JS had no way to turn an
/// uploaded image into the pixel-level `RawImage` the document model requires.
#[cfg(feature = "images")]
pub fn decode_image(input: DecodeImageInput) -> Result<DecodeImageOutput, String> {
    let bytes = input.bytes.decode_bytes()?;
    let mut warnings = Vec::new();
    let image = crate::RawImage::decode_from_bytes(&bytes, &mut warnings)?;
    Ok(DecodeImageOutput { image, warnings })
}

#[cfg(not(feature = "images"))]
pub fn decode_image(_input: DecodeImageInput) -> Result<DecodeImageOutput, String> {
    Err("Pdf_DecodeImage failed: no image format features enabled for printpdf crate".to_string())
}

pub async fn decode_image_async(input: DecodeImageInput) -> Result<DecodeImageOutput, String> {
    decode_image(input)
}
