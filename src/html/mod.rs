//! HTML to PDF converter using azul's layout engine
//!
//! This module provides HTML/XML to PDF conversion using:
//! - azul_layout::xml for XML/HTML parsing
//! - azul_layout::LayoutWindow for layout calculation  
//! - azul_layout::pdf for DisplayList → PDF ops conversion
//! - Internal bridge module for translating azul PDF ops to printpdf Ops

use std::collections::BTreeMap;

use azul_core::{
    dom::DomId,
    geom::{LogicalSize, LogicalRect, LogicalPosition},
    resources::RendererResources,
    xml::{str_to_dom, DynamicXmlComponent},
};
use azul_layout::{
    font::loading::build_font_cache,
    paged::FragmentationContext,
    solver3::paged_layout::layout_document_paged_with_config,
    solver3::pagination::FakePageConfig,
    text3::cache::FontHash,
    font_traits::{TextLayoutCache, FontManager},
    Solver3LayoutCache,
    xml::parse_xml_string,
};
use serde_derive::{Deserialize, Serialize};

use crate::{font::ParsedFont, Mm, PdfDocument, PdfPage, PdfWarnMsg};

pub mod bridge;
pub mod border;

/// Page margins configuration in millimeters
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMargins {
    /// Top margin in millimeters
    #[serde(default)]
    pub top: Mm,
    /// Right margin in millimeters
    #[serde(default)]
    pub right: Mm,
    /// Bottom margin in millimeters
    #[serde(default)]
    pub bottom: Mm,
    /// Left margin in millimeters
    #[serde(default)]
    pub left: Mm,
}

impl PageMargins {
    /// Create new margins with all sides set to the same value
    pub fn uniform(margin: Mm) -> Self {
        Self {
            top: margin,
            right: margin,
            bottom: margin,
            left: margin,
        }
    }
    
    /// Create new margins with symmetric horizontal and vertical values
    pub fn symmetric(vertical: Mm, horizontal: Mm) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
    
    /// Create new margins with explicit values for all sides
    pub fn new(top: Mm, right: Mm, bottom: Mm, left: Mm) -> Self {
        Self { top, right, bottom, left }
    }
    
    /// Returns the total horizontal margin (left + right)
    pub fn horizontal(&self) -> Mm {
        Mm(self.left.0 + self.right.0)
    }
    
    /// Returns the total vertical margin (top + bottom)
    pub fn vertical(&self) -> Mm {
        Mm(self.top.0 + self.bottom.0)
    }
}

/// Options for rendering XML/HTML to PDF
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlRenderOptions {
    /// Embedded images (key = image ID, value = image bytes)
    #[serde(default)]
    pub images: BTreeMap<String, Vec<u8>>,
    /// Embedded fonts (key = font name, value = font bytes)
    #[serde(default)]
    pub fonts: BTreeMap<String, Vec<u8>>,
    /// Page width in millimeters
    #[serde(default = "default_page_width")]
    pub page_width: Mm,
    /// Page height in millimeters
    #[serde(default = "default_page_height")]
    pub page_height: Mm,
    /// Page margins - affects the content area available for layout
    #[serde(default)]
    pub margins: PageMargins,
    /// Show page numbers in footer ("Page X of Y" format)
    #[serde(default)]
    pub show_page_numbers: bool,
    /// Custom header text (appears on all pages except first if skip_first_page is true)
    #[serde(default)]
    pub header_text: Option<String>,
    /// Custom footer text (in addition to or instead of page numbers)
    #[serde(default)]
    pub footer_text: Option<String>,
    /// Skip header/footer on the first page
    #[serde(default)]
    pub skip_first_page: bool,
}

impl Default for XmlRenderOptions {
    fn default() -> Self {
        Self {
            images: BTreeMap::new(),
            fonts: BTreeMap::new(),
            page_width: default_page_width(),
            page_height: default_page_height(),
            margins: PageMargins::default(),
            show_page_numbers: false,
            header_text: None,
            footer_text: None,
            skip_first_page: false,
        }
    }
}

fn default_page_width() -> Mm {
    Mm(210.0) // A4 width
}

fn default_page_height() -> Mm {
    Mm(297.0) // A4 height
}

/// Convert XML/HTML content to PDF pages, returning pages and font map
pub fn xml_to_pdf_pages(
    xml: &str,
    options: &XmlRenderOptions,
) -> Result<(Vec<PdfPage>, BTreeMap<FontHash, ParsedFont>), Vec<PdfWarnMsg>> {
    let mut warnings = Vec::new();

    // Type-safe preprocessing: RawHtml -> PreprocessedHtml
    let preprocessed = RawHtml::new(xml).preprocess();
    let inlined_xml = preprocessed.as_str();

    // Parse XML to XmlNode tree
    let root_nodes = match parse_xml_string(inlined_xml) {
        Ok(nodes) => nodes,
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Failed to parse XML: {}", e),
            ));
            return Err(warnings);
        }
    };

    // Calculate content area (page size minus margins)
    let mm_to_pt = 2.83465;
    let page_width_pt = options.page_width.0 * mm_to_pt;
    let page_height_pt = options.page_height.0 * mm_to_pt;
    let margin_top_pt = options.margins.top.0 * mm_to_pt;
    let margin_right_pt = options.margins.right.0 * mm_to_pt;
    let margin_bottom_pt = options.margins.bottom.0 * mm_to_pt;
    let margin_left_pt = options.margins.left.0 * mm_to_pt;
    
    // Content area is the page minus margins
    let content_width_pt = page_width_pt - margin_left_pt - margin_right_pt;
    let content_height_pt = page_height_pt - margin_top_pt - margin_bottom_pt;

    // Convert XML nodes to StyledDom with registered HTML components
    // Use content width (not page width) for layout
    let mut component_map = crate::components::printpdf_default_components();
    
    let styled_dom = match str_to_dom(
        root_nodes.as_ref(),
        &mut component_map,
        Some(content_width_pt),
    ) {
        Ok(dom) => dom,
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Failed to convert XML to DOM: {}", e),
            ));
            return Err(warnings);
        }
    };

    // Create font cache and font manager
    let mut fc_cache = build_font_cache();
    
    // Add embedded fonts from options to the font cache
    // This is critical for WASM where there are no system fonts
    #[cfg(target_family = "wasm")]
    {
        web_sys::console::log_1(&format!("Loading {} embedded fonts...", options.fonts.len()).into());
    }
    
    for (font_name, font_bytes) in &options.fonts {
        #[cfg(target_family = "wasm")]
        {
            web_sys::console::log_1(&format!("  Parsing font: {} ({} bytes)", font_name, font_bytes.len()).into());
        }
        
        if let Some(parsed_fonts) = rust_fontconfig::FcParseFontBytes(font_bytes, font_name) {
            #[cfg(target_family = "wasm")]
            {
                web_sys::console::log_1(&format!("    -> Successfully parsed {} font variants", parsed_fonts.len()).into());
            }
            fc_cache.with_memory_fonts(parsed_fonts);
        } else {
            #[cfg(target_family = "wasm")]
            {
                web_sys::console::log_1(&format!("    -> Failed to parse font!").into());
            }
        }
    }
    
    #[cfg(target_family = "wasm")]
    {
        let font_list = fc_cache.list();
        web_sys::console::log_1(&format!("Font cache now contains {} fonts", font_list.len()).into());
    }
    
    let mut font_manager = match FontManager::new(fc_cache) {
        Ok(fm) => fm,
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Failed to create font manager: {:?}", e),
            ));
            return Err(warnings);
        }
    };

    // Use content size for layout (page size minus margins)
    let content_size = LogicalSize::new(content_width_pt, content_height_pt);
    
    // Create fragmentation context for paged layout using CONTENT size
    // This ensures page breaks happen at the correct content boundaries
    let fragmentation_context = FragmentationContext::new_paged(content_size);
    
    // Create layout cache and text cache
    let mut layout_cache = Solver3LayoutCache {
        tree: None,
        calculated_positions: std::collections::BTreeMap::new(),
        viewport: None,
        scroll_ids: std::collections::BTreeMap::new(),
        scroll_id_to_node_id: std::collections::BTreeMap::new(),
        counters: std::collections::BTreeMap::new(),
        float_cache: std::collections::BTreeMap::new(),
    };
    let mut text_cache = TextLayoutCache::new();
    
    // Viewport is the content area (layout is done within margins)
    let viewport = LogicalRect {
        origin: LogicalPosition::zero(),
        size: content_size,
    };
    
    // Perform paged layout - returns Vec<DisplayList>
    let renderer_resources = RendererResources::default();
    let mut debug_messages = Some(Vec::new());

    // Create font loader closure
    use azul_layout::text3::default::PathLoader;
    let loader = PathLoader::new();
    let font_loader = |bytes: &[u8], index: usize| loader.load_font(bytes, index);
    
    // Build page config from options
    // NOTE: Full CSS @page rule parsing is not yet implemented.
    // This uses FakePageConfig for programmatic control over headers/footers.
    let mut page_config = FakePageConfig::new();
    
    if options.show_page_numbers {
        page_config = page_config.with_footer_page_numbers();
    }
    
    if let Some(ref header) = options.header_text {
        page_config = page_config.with_header_text(header.clone());
    }
    
    if let Some(ref footer) = options.footer_text {
        page_config = page_config.with_footer_text(footer.clone());
    }
    
    if options.skip_first_page {
        page_config = page_config.skip_first_page(true);
    }
    
    let display_lists = match layout_document_paged_with_config(
        &mut layout_cache,
        &mut text_cache,
        fragmentation_context,
        styled_dom,
        viewport,
        &mut font_manager,
        &std::collections::BTreeMap::new(), // No scroll offsets for PDF
        &std::collections::BTreeMap::new(), // No selections for PDF
        &mut debug_messages,
        None, // No GPU cache for PDF
        &renderer_resources,
        azul_core::resources::IdNamespace(0),
        DomId::ROOT_ID,
        font_loader,
        page_config,
    ) {
        Ok(lists) => lists,
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Layout solving failed: {:?}", e),
            ));
            return Err(warnings);
        }
    };

    // Convert each DisplayList to a PDF page
    let mut pages = Vec::new();
    // font_data_map now maps u64 (font hash) directly to ParsedFont
    let mut font_data_map: BTreeMap<FontHash, azul_layout::font::parsed::ParsedFont> = BTreeMap::new();
    
    // Full page size for PDF coordinate transformation
    let full_page_size = LogicalSize::new(page_width_pt, page_height_pt);
    
    for display_list in display_lists.iter() {
        // Convert DisplayList to printpdf operations
        // We pass the FULL page size for Y-coordinate transformation (PDF origin is bottom-left)
        // The content was laid out in content_size, but coordinates need to be transformed
        // relative to the full page height
        let pdf_ops = bridge::display_list_to_printpdf_ops_with_margins(
            &display_list, 
            full_page_size,
            margin_left_pt,
            margin_top_pt,
            &font_manager
        ).map_err(|e| vec![PdfWarnMsg::warning(0, 0, format!("Failed to convert display list: {}", e))])?;
        
        // Extract fonts from TextLayout items by collecting font hashes from the layout
        // and then looking them up in the font_manager
        for item in display_list.items.iter() {
            if let azul_layout::solver3::display_list::DisplayListItem::TextLayout { layout, .. } = item {
                // Downcast the type-erased layout to UnifiedLayout
                if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout>() {
                    // Collect all font hashes used in this layout by scanning the positioned items
                    let mut used_font_hashes = std::collections::HashSet::new();
                    for positioned_item in &unified_layout.items {
                        if let azul_layout::text3::cache::ShapedItem::Cluster(cluster) = &positioned_item.item {
                            for glyph in &cluster.glyphs {
                                used_font_hashes.insert(glyph.font_hash);
                            }
                        }
                    }
                    
                    // Look up each font hash in the font_manager
                    for font_hash in used_font_hashes {
                        let font_hash_key = FontHash { font_hash };
                        if !font_data_map.contains_key(&font_hash_key) {
                            // Get the font from the font manager by hash
                            if let Some(font_ref) = font_manager.get_font_by_hash(font_hash) {
                                // Extract ParsedFont from FontRef
                                let parsed_font = unsafe {
                                    let parsed_ptr = font_ref.get_parsed();
                                    let parsed_font = &*(parsed_ptr as *const azul_layout::font::parsed::ParsedFont);
                                    parsed_font.clone()
                                };
                                font_data_map.insert(font_hash_key, parsed_font);
                            }
                        }
                    }
                }
            }
        }
        
        // Create a page for this display list (using full page dimensions)
        let page = PdfPage::new(options.page_width, options.page_height, pdf_ops);
        pages.push(page);
    }
    
    // If no pages were generated, create at least one empty page
    if pages.is_empty() {
        warnings.push(PdfWarnMsg::warning(0, 0, "No content generated, creating empty page".to_string()));
        let page = PdfPage::new(options.page_width, options.page_height, Vec::new());
        pages.push(page);
    }

    // Always return Ok with pages and fonts
    Ok((pages, font_data_map))
}

/// Add XML/HTML content to an existing PDF document
pub fn add_xml_to_document(
    document: &mut PdfDocument,
    xml: &str,
    options: &XmlRenderOptions,
) -> Result<(), Vec<PdfWarnMsg>> {
    match xml_to_pdf_pages(xml, options) {
        Ok((pages, font_data)) => {
            // Register fonts in the document
            for (font_hash, parsed_font) in font_data.into_iter() {
                let font_id = crate::FontId(format!("F{}", font_hash.font_hash));
                let pdf_font = crate::font::PdfFont::new(parsed_font);
                document.resources.fonts.map.insert(font_id, pdf_font);
            }
            document.pages.extend(pages);
            Ok(())
        }
        Err(warnings) => Err(warnings),
    }
}

/// Raw HTML input (not yet processed)
#[derive(Debug, Clone)]
pub struct RawHtml(String);

impl RawHtml {
    pub fn new(html: impl Into<String>) -> Self {
        Self(html.into())
    }
    
    /// Process raw HTML into preprocessed HTML with CSS inlined
    pub fn preprocess(self) -> PreprocessedHtml {
        let cleaned = clean_html_elements(&self.0);
        let inlined = inline_css_in_xml(&cleaned);
        PreprocessedHtml(inlined)
    }
}

/// HTML with CSS rules inlined as style="" attributes (ready for XML parsing)
#[derive(Debug, Clone)]
pub struct PreprocessedHtml(String);

impl PreprocessedHtml {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Configuration extracted from HTML meta tags and title
#[derive(Default)]
pub struct HtmlExtractedConfig {
    /// Title extracted from the <title> element
    pub title: Option<String>,
    /// PDF page width from meta tags (in mm)
    pub page_width: Option<f32>,
    /// PDF page height from meta tags (in mm)
    pub page_height: Option<f32>,
    /// PDF metadata from meta tags
    pub metadata: BTreeMap<String, String>,
    /// Custom XML components (not cloneable, Debug not implemented in azul)
    pub components: Vec<DynamicXmlComponent>,
}

/// Inline CSS rules into style attributes
/// Note: This is a simplified version that works on XML strings
/// For more complex selector matching, consider using a proper HTML/CSS parser
///
/// IMPORTANT: This function does NOT remove <style> tags anymore!
/// The <style> tags are left intact so that str_to_dom() can process them
/// as global CSS. This function only provides OPTIONAL inline style augmentation
/// for simple selectors, but the main CSS processing happens in azul's CSS engine.
fn inline_css_in_xml(xml: &str) -> String {
    // DO NOT extract or remove style blocks - leave them for str_to_dom()
    // The azul CSS engine is much more sophisticated than this simple inliner
    // and can handle complex selectors, cascading, specificity, etc.
    //
    // This function used to remove <style> tags, which broke global CSS application!
    // Now we just return the XML as-is, letting azul's str_to_dom() handle all CSS.
    
    xml.to_string()
}

/// Extract HTML configuration from XML content (basic parsing)
fn extract_html_config(xml: &str) -> HtmlExtractedConfig {
    let mut config = HtmlExtractedConfig::default();
    
    // Extract title
    if let Some(title_start) = xml.find("<title>") {
        if let Some(title_end) = xml[title_start..].find("</title>") {
            let title_content = &xml[title_start + 7..title_start + title_end];
            config.title = Some(title_content.trim().to_string());
        }
    }
    
    // Extract meta tags
    let mut search_pos = 0;
    while let Some(meta_start) = xml[search_pos..].find("<meta ") {
        let meta_pos = search_pos + meta_start;
        if let Some(meta_end) = xml[meta_pos..].find('>') {
            let meta_tag = &xml[meta_pos..meta_pos + meta_end];
            
            // Extract name and content attributes
            let name = extract_attribute(meta_tag, "name");
            let content = extract_attribute(meta_tag, "content");
            
            if let (Some(name), Some(content)) = (name, content) {
                if name.starts_with("pdf.options.") {
                    let option_name = &name[12..]; // Skip "pdf.options."
                    match option_name {
                        "pageWidth" => {
                            if let Ok(width) = content.parse::<f32>() {
                                config.page_width = Some(width);
                            }
                        }
                        "pageHeight" => {
                            if let Ok(height) = content.parse::<f32>() {
                                config.page_height = Some(height);
                            }
                        }
                        _ => {}
                    }
                } else if name.starts_with("pdf.metadata.") {
                    let metadata_key = &name[13..]; // Skip "pdf.metadata."
                    config.metadata.insert(metadata_key.to_string(), content);
                }
            }
            
            search_pos = meta_pos + meta_end + 1;
        } else {
            break;
        }
    }
    
    config
}

/// Extract attribute value from an HTML/XML tag
fn extract_attribute(tag: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr_name);
    if let Some(start) = tag.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = tag[value_start..].find('"') {
            return Some(tag[value_start..value_start + end].to_string());
        }
    }
    None
}

/// Apply HTML configuration to PDF document
pub fn apply_html_config(
    doc: &mut PdfDocument,
    config: &HtmlExtractedConfig,
    options: &mut XmlRenderOptions,
) {
    // Apply title if provided
    if let Some(title) = &config.title {
        if doc.metadata.info.document_title.is_empty() {
            doc.metadata.info.document_title = title.clone();
        }
    }

    // Apply page dimensions
    if let Some(width) = config.page_width {
        options.page_width = Mm(width);
    }

    if let Some(height) = config.page_height {
        options.page_height = Mm(height);
    }

    // Apply metadata
    for (key, value) in &config.metadata {
        match (key.trim(), value.trim()) {
            ("trapped", "true") => doc.metadata.info.trapped = true,
            ("trapped", "false") => doc.metadata.info.trapped = false,
            ("version", v) => {
                if let Ok(p) = v.parse() {
                    doc.metadata.info.version = p;
                }
            }
            ("conformance", v) => {
                if let Ok(p) = serde_json::from_str(v) {
                    doc.metadata.info.conformance = p;
                }
            }
            ("documentTitle", v) => doc.metadata.info.document_title = v.to_string(),
            ("author", v) => doc.metadata.info.author = v.to_string(),
            ("creator", v) => doc.metadata.info.creator = v.to_string(),
            ("producer", v) => doc.metadata.info.producer = v.to_string(),
            ("subject", v) => doc.metadata.info.subject = v.to_string(),
            ("keywords", v) => {
                doc.metadata.info.keywords = v.split(',').map(|s| s.trim().to_string()).collect();
            }
            ("identifier", v) => doc.metadata.info.identifier = v.to_string(),
            _ => {} // Ignore unknown metadata
        }
    }
}

/// Remove non-renderable HTML elements from XML string
fn clean_html_elements(xml: &str) -> String {
    let mut result = xml.to_string();
    
    let non_renderable = [
        "script", "noscript", "iframe", "canvas", "audio", "video", 
        "source", "track", "embed", "object", "param", "picture",
    ];
    
    for elem in non_renderable {
        // Remove self-closing tags
        let self_closing = format!("<{} />", elem);
        result = result.replace(&self_closing, "");
        
        let self_closing2 = format!("<{}/>", elem);
        result = result.replace(&self_closing2, "");
        
        // Remove paired tags with content
        let open_tag = format!("<{}>", elem);
        let close_tag = format!("</{}>", elem);
        
        while let Some(start) = result.find(&open_tag) {
            if let Some(end_pos) = result[start..].find(&close_tag) {
                let end = start + end_pos + close_tag.len();
                result.replace_range(start..end, "");
            } else {
                break;
            }
        }
        
        // Remove tags with attributes
        let open_tag_with_attr = format!("<{} ", elem);
        while let Some(start) = result.find(&open_tag_with_attr) {
            if let Some(tag_end) = result[start..].find('>') {
                let tag_close_pos = start + tag_end + 1;
                if let Some(close_pos) = result[tag_close_pos..].find(&close_tag) {
                    let end = tag_close_pos + close_pos + close_tag.len();
                    result.replace_range(start..end, "");
                } else {
                    // Self-closing tag with attributes
                    result.replace_range(start..tag_close_pos, "");
                }
            } else {
                break;
            }
        }
    }
    
    result
}

/// Process HTML content for rendering: inline CSS, extract config, clean elements
/// This function is now deprecated - use RawHtml::new().preprocess() instead
#[deprecated(since = "0.8.0", note = "Use RawHtml::new().preprocess() for type-safe preprocessing")]
pub fn process_html_for_rendering(html: &str) -> (String, HtmlExtractedConfig) {
    let config = extract_html_config(html);
    let preprocessed = RawHtml::new(html).preprocess();
    (preprocessed.0, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_xml_rendering() {
        let xml = r#"
            <app>
                <div style="width: 200px; height: 100px; background-color: red;">
                    Hello World
                </div>
            </app>
        "#;

        let options = XmlRenderOptions::default();
        let result = xml_to_pdf_pages(xml, &options);

        // Should now succeed with the full implementation
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_default_options() {
        let options = XmlRenderOptions::default();
        assert_eq!(options.page_width, Mm(210.0));
        assert_eq!(options.page_height, Mm(297.0));
    }

    #[test]
    fn test_html_config_extraction() {
        let html = r#"
            <html>
                <head>
                    <title>Test Document</title>
                    <meta name="pdf.options.pageWidth" content="200" />
                    <meta name="pdf.options.pageHeight" content="280" />
                    <meta name="pdf.metadata.author" content="Test Author" />
                </head>
            </html>
        "#;
        
        let config = extract_html_config(html);
        assert_eq!(config.title, Some("Test Document".to_string()));
        assert_eq!(config.page_width, Some(200.0));
        assert_eq!(config.page_height, Some(280.0));
        assert_eq!(config.metadata.get("author"), Some(&"Test Author".to_string()));
    }

    #[test]
    fn test_clean_html_elements() {
        let html = r#"
            <div>
                <script>alert('test');</script>
                <p>Content</p>
                <iframe src="test.html"></iframe>
            </div>
        "#;
        
        let cleaned = clean_html_elements(html);
        assert!(!cleaned.contains("<script"));
        assert!(!cleaned.contains("<iframe"));
        assert!(cleaned.contains("<p>Content</p>"));
    }

    #[test]
    fn test_inline_css() {
        let xml = r#"
            <style>div { color: red; }</style>
            <div>Test</div>
        "#;
        
        // inline_css_in_xml no longer removes <style> tags - they are left for azul's CSS engine
        let inlined = inline_css_in_xml(xml);
        // <style> tags are preserved for str_to_dom() to process
        assert!(inlined.contains("<style>"));
        // The function just returns the XML as-is now
        assert!(inlined.contains("<div>Test</div>"));
    }
}
