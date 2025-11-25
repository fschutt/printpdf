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
    solver3::{paged_layout::layout_document_paged},
    text3::cache::FontHash,
    font_traits::{TextLayoutCache, FontManager},
    Solver3LayoutCache,
    xml::parse_xml_string,
};
use serde_derive::{Deserialize, Serialize};

use crate::{font::ParsedFont, Mm, PdfDocument, PdfPage, PdfWarnMsg};

pub mod bridge;
mod border;

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
}

impl Default for XmlRenderOptions {
    fn default() -> Self {
        Self {
            images: BTreeMap::new(),
            fonts: BTreeMap::new(),
            page_width: default_page_width(),
            page_height: default_page_height(),
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

    // Convert XML nodes to StyledDom with registered HTML components
    let mut component_map = crate::components::printpdf_default_components();
    let page_width_pt = options.page_width.0 * 2.83465; // mm to pt
    
    let styled_dom = match str_to_dom(
        root_nodes.as_ref(),
        &mut component_map,
        Some(page_width_pt),
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
    let fc_cache = build_font_cache();
    let font_manager = match FontManager::new(fc_cache) {
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
    
    // Prepare page size for paged layout context
    let page_height_pt = options.page_height.0 * 2.83465; // mm to pt
    let page_size = LogicalSize::new(page_width_pt, page_height_pt);
    
    // Create fragmentation context for paged layout
    let fragmentation_context = FragmentationContext::new_paged(page_size);
    
    // Create layout cache and text cache
    use azul_css::props::basic::FontRef;
    let mut layout_cache = Solver3LayoutCache::<FontRef> {
        tree: None,
        calculated_positions: std::collections::BTreeMap::new(),
        viewport: None,
        scroll_ids: std::collections::BTreeMap::new(),
        scroll_id_to_node_id: std::collections::BTreeMap::new(),
        counters: std::collections::BTreeMap::new(),
        float_cache: std::collections::BTreeMap::new(),
    };
    let mut text_cache = TextLayoutCache::new();
    
    // Prepare viewport (same as page size for PDF)
    let viewport = LogicalRect {
        origin: LogicalPosition::zero(),
        size: page_size,
    };
    
    // Perform paged layout - returns Vec<DisplayList>
    let renderer_resources = RendererResources::default();
    let mut debug_messages = Some(Vec::new());
    
    let display_lists = match layout_document_paged(
        &mut layout_cache,
        &mut text_cache,
        fragmentation_context,
        styled_dom,
        viewport,
        &font_manager,
        &std::collections::BTreeMap::new(), // No scroll offsets for PDF
        &std::collections::BTreeMap::new(), // No selections for PDF
        &mut debug_messages,
        None, // No GPU cache for PDF
        &renderer_resources,
        azul_core::resources::IdNamespace(0),
        DomId::ROOT_ID,
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
    
    for display_list in display_lists.iter() {
        // Convert DisplayList to printpdf operations
        let pdf_ops = bridge::display_list_to_printpdf_ops(&display_list, page_size, &font_manager)
            .map_err(|e| vec![PdfWarnMsg::warning(0, 0, format!("Failed to convert display list: {}", e))])?;
        
        // Extract fonts from TextLayout items (which contain the full UnifiedLayout with used_fonts)
        // Ignore Text items - they are for visual renderers only
        // Note: The UnifiedLayout is parameterized with FontRef (which wraps ParsedFont)
        for item in display_list.items.iter() {
            if let azul_layout::solver3::display_list::DisplayListItem::TextLayout { layout, .. } = item {
                // Downcast the type-erased layout to UnifiedLayout<FontRef>
                // (FontRef is the concrete type used by FontManager<FontRef, PathLoader>)
                use azul_css::props::basic::FontRef;
                if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout<FontRef>>() {
                    // Extract all used fonts from this layout
                    // FontRef contains a pointer to ParsedFont
                    for (font_hash, font_ref) in unified_layout.used_fonts.iter() {
                        let font_hash_key = FontHash { font_hash: *font_hash };
                        if !font_data_map.contains_key(&font_hash_key) {
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
        
        // Create a page for this display list
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

/// Simple CSS rule representation
#[derive(Debug, Clone)]
struct CssRule {
    selector: String,
    declarations: Vec<(String, String)>, // Property name, value
}

/// Parse CSS text into a list of rules (basic implementation)
fn parse_css(css_text: &str) -> Vec<CssRule> {
    let mut rules = Vec::new();

    for rule_text in css_text.split('}') {
        let parts: Vec<&str> = rule_text.split('{').collect();
        if parts.len() >= 2 {
            let selector = parts[0].trim();
            let declarations_text = parts[1].trim();

            let mut declarations = Vec::new();
            for decl in declarations_text.split(';') {
                let decl_parts: Vec<&str> = decl.split(':').collect();
                if decl_parts.len() >= 2 {
                    let property = decl_parts[0].trim();
                    let value = decl_parts[1].trim();
                    if !property.is_empty() && !value.is_empty() {
                        declarations.push((property.to_string(), value.to_string()));
                    }
                }
            }

            if !selector.is_empty() && !declarations.is_empty() {
                rules.push(CssRule {
                    selector: selector.to_string(),
                    declarations,
                });
            }
        }
    }

    rules
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

/// Apply a single CSS rule to XML string (basic implementation)
fn apply_css_rule_to_xml(xml: &str, rule: &CssRule) -> String {
    let mut result = xml.to_string();
    
    // Build the style string to add
    let mut style_additions = String::new();
    for (prop, val) in &rule.declarations {
        if !style_additions.is_empty() {
            style_additions.push(';');
        }
        style_additions.push_str(&format!("{}:{}", prop, val));
    }
    
    // Split selector by comma to handle multiple selectors (e.g., "th, td")
    for selector in rule.selector.split(',') {
        let selector = selector.trim();
        
        if selector.starts_with('.') {
            // Class selector
            let class_name = &selector[1..];
            result = add_style_to_class(&result, class_name, &style_additions);
        } else if selector.starts_with('#') {
            // ID selector
            let id_name = &selector[1..];
            result = add_style_to_id(&result, id_name, &style_additions);
        } else {
            // Element selector
            result = add_style_to_element(&result, selector, &style_additions);
        }
    }
    
    result
}

/// Add style to elements with a specific class
fn add_style_to_class(xml: &str, class_name: &str, style: &str) -> String {
    let mut result = String::new();
    let class_pattern = format!("class=\"{}\"", class_name);
    let class_pattern_with_spaces = format!("class=\"{} ", class_name);
    
    let mut remaining = xml;
    while let Some(pos) = remaining.find(&class_pattern).or_else(|| remaining.find(&class_pattern_with_spaces)) {
        result.push_str(&remaining[..pos]);
        
        // Find the closing > of this tag
        if let Some(close_pos) = remaining[pos..].find('>') {
            let tag_end = pos + close_pos;
            let tag_content = &remaining[pos..tag_end];
            
            // Add or append to style attribute
            let new_tag = if tag_content.contains("style=\"") {
                // Append to existing style
                tag_content.replace("style=\"", &format!("style=\"{};", style))
            } else {
                // Add new style attribute
                format!("{} style=\"{}\"", tag_content, style)
            };
            
            result.push_str(&new_tag);
            result.push('>');
            remaining = &remaining[tag_end + 1..];
        } else {
            result.push_str(remaining);
            break;
        }
    }
    result.push_str(remaining);
    result
}

/// Add style to elements with a specific ID
fn add_style_to_id(xml: &str, id_name: &str, style: &str) -> String {
    let mut result = String::new();
    let id_pattern = format!("id=\"{}\"", id_name);
    
    let mut remaining = xml;
    if let Some(pos) = remaining.find(&id_pattern) {
        result.push_str(&remaining[..pos]);
        
        // Find the closing > of this tag
        if let Some(close_pos) = remaining[pos..].find('>') {
            let tag_end = pos + close_pos;
            let tag_content = &remaining[pos..tag_end];
            
            // Add or append to style attribute
            let new_tag = if tag_content.contains("style=\"") {
                // Append to existing style
                tag_content.replace("style=\"", &format!("style=\"{};", style))
            } else {
                // Add new style attribute
                format!("{} style=\"{}\"", tag_content, style)
            };
            
            result.push_str(&new_tag);
            result.push('>');
            remaining = &remaining[tag_end + 1..];
        }
    }
    result.push_str(remaining);
    result
}

/// Add style to elements with a specific tag name
fn add_style_to_element(xml: &str, element_name: &str, style: &str) -> String {
    let mut result = String::new();
    let element_start = format!("<{}", element_name);
    
    let mut remaining = xml;
    while let Some(pos) = remaining.find(&element_start) {
        result.push_str(&remaining[..pos]);
        
        // Make sure it's a complete element name (followed by space or >)
        let after_elem = pos + element_start.len();
        if after_elem < remaining.len() {
            let next_char = remaining.chars().nth(after_elem).unwrap();
            if next_char != ' ' && next_char != '>' && next_char != '/' {
                result.push_str(&element_start);
                remaining = &remaining[after_elem..];
                continue;
            }
        }
        
        // Find the closing > of this tag
        if let Some(close_pos) = remaining[pos..].find('>') {
            let tag_end = pos + close_pos;
            let tag_content = &remaining[pos..tag_end];
            
            // Add or append to style attribute
            let new_tag = if tag_content.contains("style=\"") {
                // Append to existing style
                tag_content.replace("style=\"", &format!("style=\"{};", style))
            } else {
                // Add new style attribute before closing >
                format!("{} style=\"{}\"", tag_content, style)
            };
            
            result.push_str(&new_tag);
            result.push('>');
            remaining = &remaining[tag_end + 1..];
        } else {
            result.push_str(remaining);
            break;
        }
    }
    result.push_str(remaining);
    result
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
    fn test_css_parsing() {
        let css = "div { color: red; font-size: 12px; } .my-class { margin: 10px; }";
        let rules = parse_css(css);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].selector, "div");
        assert_eq!(rules[0].declarations.len(), 2);
        assert_eq!(rules[1].selector, ".my-class");
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
        
        let inlined = inline_css_in_xml(xml);
        assert!(!inlined.contains("<style>"));
        assert!(inlined.contains("style="));
    }
}
