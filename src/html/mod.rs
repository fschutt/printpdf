//! HTML to PDF converter using azul's layout engine
//!
//! This module provides HTML/XML to PDF conversion using:
//! - azul_layout::xml for XML/HTML parsing
//! - azul_layout::LayoutWindow for layout calculation  
//! - azul_layout::pdf for DisplayList → PDF ops conversion
//! - Internal bridge module for translating azul PDF ops to printpdf Ops

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

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

/// Shared font state that can be reused across multiple `xml_to_pdf_pages` calls.
///
/// Holds both the fontconfig cache (system font paths / metadata) and
/// the already-parsed font data so that font files are only read and
/// parsed once, even when rendering many documents.
///
/// Build with [`build_font_pool`], then pass via [`XmlRenderOptions::font_pool`].
#[derive(Clone, Debug)]
pub struct SharedFontPool {
    /// Font-path / metadata cache (shared, read-only after build).
    pub fc_cache: Arc<rust_fontconfig::FcFontCache>,
    /// Pool of already-parsed font binaries keyed by FontId.
    /// Populated lazily during the first layout pass and reused
    /// by every subsequent pass that shares this pool.
    pub parsed_fonts: Arc<Mutex<HashMap<rust_fontconfig::FontId, azul_css::props::basic::FontRef>>>,
}

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
    /// Shared font pool to reuse across multiple `xml_to_pdf_pages` calls.
    ///
    /// Holds both the fontconfig metadata cache *and* the already-parsed
    /// font binaries so that font files are read from disk only once.
    ///
    /// Build with [`build_font_pool`], then set this field.
    /// On the first render the pool will load fonts lazily; subsequent
    /// renders reuse the cached parse results (~0 ms for font loading).
    #[serde(skip)]
    pub font_pool: Option<SharedFontPool>,
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
            font_pool: None,
        }
    }
}

/// Build a [`SharedFontPool`] suitable for reuse across many render calls.
///
/// Scans system fonts (or returns an empty cache on WASM), registers any
/// embedded fonts from `fonts`, and returns a pool whose `parsed_fonts`
/// map starts empty — it will be populated lazily on the first layout.
///
/// # Arguments
/// * `fonts` — Embedded fonts to register (same map you'd put in `XmlRenderOptions::fonts`)
/// * `families` — If `Some`, only scan system fonts matching these family names
///   (e.g. `&["monospace"]`). Pass `None` to scan all system fonts.
pub fn build_font_pool(
    fonts: &BTreeMap<String, Vec<u8>>,
    families: Option<&[&str]>,
) -> SharedFontPool {
    let mut fc_cache = match families {
        Some(fams) => rust_fontconfig::FcFontCache::build_with_families(fams),
        None => build_font_cache(),
    };

    for (font_name, font_bytes) in fonts {
        if let Some(parsed_fonts) = rust_fontconfig::FcParseFontBytes(font_bytes, font_name) {
            fc_cache.with_memory_fonts(parsed_fonts);
        }
    }

    SharedFontPool {
        fc_cache: Arc::new(fc_cache),
        parsed_fonts: Arc::new(Mutex::new(HashMap::new())),
    }
}

/// Build a font cache suitable for use with [`XmlRenderOptions::font_pool`].
///
/// **Deprecated**: prefer [`build_font_pool`] which also shares parsed fonts.
/// This function only builds the fontconfig metadata cache.
pub fn build_font_cache_for_options(
    fonts: &BTreeMap<String, Vec<u8>>,
    families: Option<&[&str]>,
) -> Arc<rust_fontconfig::FcFontCache> {
    let pool = build_font_pool(fonts, families);
    pool.fc_cache
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
    let _t_total = std::time::Instant::now();

    // Type-safe preprocessing: RawHtml -> PreprocessedHtml
    let _t0 = std::time::Instant::now();
    let preprocessed = RawHtml::new(xml).preprocess();
    let inlined_xml = preprocessed.as_str();
    eprintln!("  [xml_to_pdf_pages] preprocess HTML: {:?} (html len = {} bytes)", _t0.elapsed(), xml.len());

    // Parse XML to XmlNode tree
    let _t1 = std::time::Instant::now();
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
    eprintln!("  [xml_to_pdf_pages] parse XML: {:?}", _t1.elapsed());

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

    // CRITICAL: Convert content dimensions from PDF points to CSS pixels.
    // The CSS resolver converts unit values to CSS px (e.g. 12pt → 16px via PT_TO_PX=96/72).
    // The layout engine is unit-agnostic, so if we feed it pt-valued dimensions,
    // CSS pt values get inflated by 96/72 relative to the coordinate system.
    // Solution: express the viewport in CSS px so all CSS units resolve correctly.
    // After layout, the bridge converts coordinates back from CSS px to PDF pt.
    const PT_TO_CSS_PX: f32 = 96.0 / 72.0;
    let content_width_px = content_width_pt * PT_TO_CSS_PX;
    let content_height_px = content_height_pt * PT_TO_CSS_PX;

    // Convert XML nodes to StyledDom with registered HTML components
    // Use content width in CSS px (not pt) for layout
    let _t2 = std::time::Instant::now();
    let mut component_map = crate::components::printpdf_default_components();
    
    let styled_dom = match str_to_dom(
        root_nodes.as_ref(),
        &mut component_map,
        Some(content_width_px),
    ) {
        Ok(dom) => dom,
        Err(e) => {
            eprintln!("  [xml_to_pdf_pages] str_to_dom FAILED: {}", e);
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Failed to convert XML to DOM: {}", e),
            ));
            return Err(warnings);
        }
    };
    eprintln!("  [xml_to_pdf_pages] str_to_dom: {:?}", _t2.elapsed());

    // Create font cache and font manager
    // If a shared font pool was provided, reuse both metadata and parsed fonts.
    // Otherwise build from scratch (scanning system fonts + embedded fonts).
    let _t3 = std::time::Instant::now();
    let (fc_cache_arc, parsed_fonts_arc) = if let Some(ref pool) = options.font_pool {
        eprintln!("  [xml_to_pdf_pages] reusing shared font pool");
        (Arc::clone(&pool.fc_cache), Arc::clone(&pool.parsed_fonts))
    } else {
        let pool = build_font_pool(&options.fonts, None);
        (pool.fc_cache, pool.parsed_fonts)
    };
    eprintln!("  [xml_to_pdf_pages] font cache ready: {:?}", _t3.elapsed());
    
    let mut font_manager = match FontManager::from_arc_shared(fc_cache_arc, parsed_fonts_arc) {
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

    // Use content size in CSS px for layout (converted from pt above)
    let content_size = LogicalSize::new(content_width_px, content_height_px);
    
    // Create fragmentation context for paged layout using CONTENT size (in CSS px)
    // Page breaks happen at CSS px boundaries; the bridge converts back to pt.
    let fragmentation_context = FragmentationContext::new_paged(content_size);
    
    // Create layout cache and text cache
    let mut layout_cache = Solver3LayoutCache {
        tree: None,
        calculated_positions: Vec::new(),
        viewport: None,
        scroll_ids: std::collections::BTreeMap::new(),
        scroll_id_to_node_id: std::collections::BTreeMap::new(),
        counters: std::collections::BTreeMap::new(),
        float_cache: std::collections::BTreeMap::new(),
        cache_map: Default::default(),
    };
    let mut text_cache = TextLayoutCache::new();
    
    // Viewport is the content area in CSS px (layout is done within margins)
    let viewport = LogicalRect {
        origin: LogicalPosition::zero(),
        size: content_size,
    };
    
    // Perform paged layout - returns Vec<DisplayList>
    let renderer_resources = RendererResources::default();
    let mut debug_messages = None; // None = skip debug format! overhead

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
    
    let _t5 = std::time::Instant::now();
    let display_lists = match layout_document_paged_with_config(
        &mut layout_cache,
        &mut text_cache,
        fragmentation_context,
        &styled_dom,
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
        azul_core::task::GetSystemTimeCallback { cb: azul_core::task::get_system_time_libstd },
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
    eprintln!("  [xml_to_pdf_pages] layout_document_paged: {:?} ({} pages)", _t5.elapsed(), display_lists.len());

    // Convert each DisplayList to a PDF page
    let mut pages = Vec::new();
    // font_data_map now maps u64 (font hash) directly to ParsedFont
    let mut font_data_map: BTreeMap<FontHash, azul_layout::font::parsed::ParsedFont> = BTreeMap::new();
    
    // Full page size for PDF coordinate transformation
    let full_page_size = LogicalSize::new(page_width_pt, page_height_pt);
    
    for display_list in display_lists.iter() {
        // Skip pages that have no meaningful content (only background fills)
        // A page needs at least one TextLayout item to be considered "real"
        let has_text_content = display_list.items.iter().any(|item| {
            matches!(item, azul_layout::solver3::display_list::DisplayListItem::TextLayout { .. })
        });
        
        if !has_text_content {
            // Skip this page - it only contains background rectangles
            continue;
        }
        
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

    eprintln!("  [xml_to_pdf_pages] display_list->ops+fonts: {} pages", pages.len());
    eprintln!("  [xml_to_pdf_pages] TOTAL: {:?}", _t_total.elapsed());

    // Always return Ok with pages and fonts
    Ok((pages, font_data_map))
}

/// Debug information from PDF generation
#[derive(Debug, Clone)]
pub struct PdfDebugInfo {
    /// The display lists for each page (before conversion to PDF ops)
    pub display_list_debug: Vec<String>,
    /// The PDF operations for each page (after conversion)
    pub pdf_ops_debug: Vec<String>,
}

/// Convert XML/HTML content to PDF pages with debug information.
/// This is the same as `xml_to_pdf_pages` but also returns debug info about
/// the display list and PDF operations for the first page.
pub fn xml_to_pdf_pages_debug(
    xml: &str,
    options: &XmlRenderOptions,
) -> Result<(Vec<PdfPage>, BTreeMap<FontHash, ParsedFont>, PdfDebugInfo), Vec<PdfWarnMsg>> {
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Starting, xml length={}", xml.len());
    let mut warnings = Vec::new();
    let mut debug_info = PdfDebugInfo {
        display_list_debug: Vec::new(),
        pdf_ops_debug: Vec::new(),
    };

    // Type-safe preprocessing: RawHtml -> PreprocessedHtml
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Preprocessing HTML...");
    let preprocessed = RawHtml::new(xml).preprocess();
    let inlined_xml = preprocessed.as_str();
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Preprocessed, length={}", inlined_xml.len());

    // Parse XML to XmlNode tree
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Parsing XML...");
    let xml_parse_start = std::time::Instant::now();
    let root_nodes = match parse_xml_string(inlined_xml) {
        Ok(nodes) => {
            eprintln!("[DEBUG xml_to_pdf_pages_debug] XML parsed in {:?}, got {} root nodes", xml_parse_start.elapsed(), nodes.len());
            nodes
        },
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

    // CRITICAL: Convert content dimensions from PDF points to CSS pixels.
    // See xml_to_pdf_pages() for detailed explanation.
    const PT_TO_CSS_PX: f32 = 96.0 / 72.0;
    let content_width_px = content_width_pt * PT_TO_CSS_PX;
    let content_height_px = content_height_pt * PT_TO_CSS_PX;

    // Convert XML nodes to StyledDom with registered HTML components
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Converting to StyledDom...");
    let str_to_dom_start = std::time::Instant::now();
    // Use content width in CSS px (not pt) for layout
    let mut component_map = crate::components::printpdf_default_components();
    
    let styled_dom = match str_to_dom(
        root_nodes.as_ref(),
        &mut component_map,
        Some(content_width_px),
    ) {
        Ok(dom) => {
            eprintln!("[DEBUG xml_to_pdf_pages_debug] StyledDom created with {} nodes in {:?}", dom.node_data.as_container().len(), str_to_dom_start.elapsed());
            dom
        },
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Failed to convert XML to DOM: {}", e),
            ));
            return Err(warnings);
        }
    };

    // Create font cache and font manager (reuse shared font pool if provided)
    let fc_start = std::time::Instant::now();
    let (fc_cache_arc, parsed_fonts_arc) = if let Some(ref pool) = options.font_pool {
        eprintln!("[DEBUG xml_to_pdf_pages_debug] Reusing shared font pool");
        (Arc::clone(&pool.fc_cache), Arc::clone(&pool.parsed_fonts))
    } else {
        eprintln!("[DEBUG xml_to_pdf_pages_debug] Building font pool from scratch...");
        let pool = build_font_pool(&options.fonts, None);
        (pool.fc_cache, pool.parsed_fonts)
    };
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Font pool ready in {:?}", fc_start.elapsed());
    
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Creating font manager...");
    let font_manager_start = std::time::Instant::now();
    let mut font_manager = match FontManager::from_arc_shared(fc_cache_arc, parsed_fonts_arc) {
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
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Font manager created in {:?}", font_manager_start.elapsed());

    // Use content size in CSS px for layout (converted from pt above)
    let content_size = LogicalSize::new(content_width_px, content_height_px);
    
    // Create fragmentation context for paged layout using CONTENT size (in CSS px)
    let fragmentation_context = FragmentationContext::new_paged(content_size);
    
    // Create layout cache and text cache
    let mut layout_cache = Solver3LayoutCache {
        tree: None,
        calculated_positions: Vec::new(),
        viewport: None,
        scroll_ids: std::collections::BTreeMap::new(),
        scroll_id_to_node_id: std::collections::BTreeMap::new(),
        counters: std::collections::BTreeMap::new(),
        float_cache: std::collections::BTreeMap::new(),
        cache_map: Default::default(),
    };
    let mut text_cache = TextLayoutCache::new();
    
    // Viewport is the content area in CSS px (layout is done within margins)
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
    
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Starting paged layout...");
    let layout_start = std::time::Instant::now();
    let display_lists = match layout_document_paged_with_config(
        &mut layout_cache,
        &mut text_cache,
        fragmentation_context,
        &styled_dom,
        viewport,
        &mut font_manager,
        &std::collections::BTreeMap::new(),
        &std::collections::BTreeMap::new(),
        &mut debug_messages,
        None,
        &renderer_resources,
        azul_core::resources::IdNamespace(0),
        DomId::ROOT_ID,
        font_loader,
        page_config,
        azul_core::task::GetSystemTimeCallback { cb: azul_core::task::get_system_time_libstd },
    ) {
        Ok(lists) => {
            eprintln!("[DEBUG xml_to_pdf_pages_debug] Paged layout completed in {:?}, got {} pages", layout_start.elapsed(), lists.len());
            lists
        },
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                0,
                0,
                format!("Layout solving failed: {:?}", e),
            ));
            return Err(warnings);
        }
    };

    // Debug: Dump layout tree and calculated positions
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Converting display lists to PDF...");
    let pdf_convert_start = std::time::Instant::now();
    {
        let mut tree_debug = String::new();
        tree_debug.push_str("=== Layout Tree Debug ===\n\n");
        
        if let Some(ref tree) = layout_cache.tree {
            tree_debug.push_str(&format!("Total nodes: {}\n", tree.nodes.len()));
            tree_debug.push_str(&format!("Calculated positions: {}\n\n", layout_cache.calculated_positions.len()));
            
            // Dump first 100 nodes with their positions and formatting context
            for (idx, node) in tree.nodes.iter().enumerate().take(100) {
                let pos = layout_cache.calculated_positions.get(idx);
                let dom_id_str = node.dom_node_id
                    .map(|id| format!("DOM#{}", id.index()))
                    .unwrap_or_else(|| "anonymous".to_string());
                
                tree_debug.push_str(&format!(
                    "Node {}: {} fc={:?} pos={:?} size={:?} inline_result={}\n",
                    idx,
                    dom_id_str,
                    node.formatting_context,
                    pos,
                    node.used_size,
                    node.inline_layout_result.is_some()
                ));
                
                // Show children
                if !node.children.is_empty() {
                    tree_debug.push_str(&format!("  children: {:?}\n", node.children));
                }
            }
        } else {
            tree_debug.push_str("Layout tree is None!\n");
        }
        
        debug_info.display_list_debug.push(tree_debug);
    }

    // Convert each DisplayList to a PDF page
    let mut pages = Vec::new();
    let mut font_data_map: BTreeMap<FontHash, azul_layout::font::parsed::ParsedFont> = BTreeMap::new();
    
    // Full page size for PDF coordinate transformation
    let full_page_size = LogicalSize::new(page_width_pt, page_height_pt);
    
    for (page_idx, display_list) in display_lists.iter().enumerate() {
        // Debug: capture display list info for first page only
        if page_idx == 0 {
            let mut dl_debug = String::new();
            dl_debug.push_str(&format!("=== Display List for Page {} ===\n", page_idx + 1));
            dl_debug.push_str(&format!("Total items: {}\n\n", display_list.items.len()));
            
            // Count item types
            let mut text_layout_count = 0;
            let mut text_count = 0;
            let mut rect_count = 0;
            let mut other_count = 0;
            
            for (item_idx, item) in display_list.items.iter().enumerate() {
                use azul_layout::solver3::display_list::DisplayListItem;
                match item {
                    DisplayListItem::TextLayout { bounds, font_hash: _, font_size_px, color, .. } => {
                        text_layout_count += 1;
                        dl_debug.push_str(&format!("Item {}: TextLayout at ({:.1}, {:.1}) size {:.1}x{:.1} font_size={:.1} color=({},{},{},{})\n", 
                            item_idx, bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height,
                            font_size_px, color.r, color.g, color.b, color.a));
                    }
                    DisplayListItem::Text { glyphs, font_size_px, color, .. } => {
                        text_count += 1;
                        dl_debug.push_str(&format!("Item {}: Text with {} glyphs, font_size={:.1} color=({},{},{},{})\n", 
                            item_idx, glyphs.len(), font_size_px, color.r, color.g, color.b, color.a));
                    }
                    DisplayListItem::Rect { bounds, color, .. } => {
                        rect_count += 1;
                        if item_idx < 20 || bounds.size.width > 100.0 {
                            dl_debug.push_str(&format!("Item {}: Rect at ({:.1}, {:.1}) size {:.1}x{:.1} color=({},{},{},{})\n", 
                                item_idx, bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height,
                                color.r, color.g, color.b, color.a));
                        }
                    }
                    _ => {
                        other_count += 1;
                    }
                }
            }
            dl_debug.push_str(&format!("\n=== Summary: {} TextLayout, {} Text, {} Rect, {} other ===\n", 
                text_layout_count, text_count, rect_count, other_count));
            debug_info.display_list_debug.push(dl_debug);
        }
        
        // Convert DisplayList to printpdf operations
        let pdf_ops = bridge::display_list_to_printpdf_ops_with_margins(
            &display_list, 
            full_page_size,
            margin_left_pt,
            margin_top_pt,
            &font_manager
        ).map_err(|e| vec![PdfWarnMsg::warning(0, 0, format!("Failed to convert display list: {}", e))])?;
        
        // Debug: capture PDF ops for first page only
        if page_idx == 0 {
            let mut ops_debug = String::new();
            ops_debug.push_str(&format!("=== PDF Ops for Page {} ===\n", page_idx + 1));
            ops_debug.push_str(&format!("Total ops: {}\n\n", pdf_ops.len()));
            
            for (op_idx, op) in pdf_ops.iter().enumerate() {
                ops_debug.push_str(&format!("Op {}: {:?}\n", op_idx, op));
            }
            debug_info.pdf_ops_debug.push(ops_debug);
        }
        
        // Extract fonts from TextLayout items
        for item in display_list.items.iter() {
            if let azul_layout::solver3::display_list::DisplayListItem::TextLayout { layout, .. } = item {
                if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout>() {
                    let mut used_font_hashes = std::collections::HashSet::new();
                    for positioned_item in &unified_layout.items {
                        if let azul_layout::text3::cache::ShapedItem::Cluster(cluster) = &positioned_item.item {
                            for glyph in &cluster.glyphs {
                                used_font_hashes.insert(glyph.font_hash);
                            }
                        }
                    }
                    
                    for font_hash in used_font_hashes {
                        let font_hash_key = FontHash { font_hash };
                        if !font_data_map.contains_key(&font_hash_key) {
                            if let Some(font_ref) = font_manager.get_font_by_hash(font_hash) {
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
        
        // Create a page for this display list
        let page = PdfPage::new(options.page_width, options.page_height, pdf_ops);
        pages.push(page);
    }
    eprintln!("[DEBUG xml_to_pdf_pages_debug] Display lists converted to {} pages in {:?}", pages.len(), pdf_convert_start.elapsed());
    
    // If no pages were generated, create at least one empty page
    if pages.is_empty() {
        warnings.push(PdfWarnMsg::warning(0, 0, "No content generated, creating empty page".to_string()));
        let page = PdfPage::new(options.page_width, options.page_height, Vec::new());
        pages.push(page);
    }

    Ok((pages, font_data_map, debug_info))
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
