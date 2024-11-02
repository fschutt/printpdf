use std::collections::BTreeMap;

use azul_core::{app_resources::{DpiScaleFactor, Epoch, IdNamespace, ImageCache, RendererResources}, callbacks::DocumentId, display_list::{DisplayListMsg, LayoutRectContent, RenderCallbacks, SolvedLayout}, styled_dom::{DomId, StyledDom}, ui_solver::LayoutResult, window::{FullWindowState, LogicalSize}, xml::{find_node_by_type, get_body_node, XmlComponentMap, XmlNode}};
use azul_css::FloatValue;
use crate::{Mm, Op, PdfDocument, PdfPage};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct XmlRenderOptions {
    pub images: BTreeMap<String, Vec<u8>>,
    pub fonts: BTreeMap<String, Vec<u8>>,
    pub page_width: Mm,
    pub page_height: Mm,
}

impl Default for XmlRenderOptions {
    fn default() -> Self {
        Self { 
            images: Default::default(), 
            fonts: Default::default(), 
            page_width: Mm(210.0), 
            page_height: Mm(297.0) 
        }
    }
}

pub(crate) fn xml_to_pages(
    file_contents: &str, 
    config: &XmlRenderOptions,
    document: &mut PdfDocument,
) -> Result<Vec<PdfPage>, String> {

    let size = LogicalSize::new(
        config.page_width.into_pt().0, 
        config.page_height.into_pt().0
    );

    let root_nodes = azulc_lib::xml::parse_xml_string(&fixup_xml(file_contents))
    .map_err(|e| format!("Error parsing XML: {}", e.to_string()))?;

    let fixup = fixup_xml_nodes(&root_nodes);

    let styled_dom = azul_core::xml::str_to_dom(
        fixup.as_ref(), 
        &mut XmlComponentMap::default()
    )
    .map_err(|e| format!("Error constructing DOM: {}", e.to_string()))?;

    let epoch = Epoch::new();
    let document_id = DocumentId {
        namespace_id: IdNamespace(0),
        id: 0,
    };
    let dom_id = DomId { inner: 0 };
    let mut fake_window_state = FullWindowState::default();
    fake_window_state.size.dimensions = size;
    let mut renderer_resources = RendererResources::default();
    let image_cache = ImageCache::default();
    let layout = solve_layout(
        styled_dom, 
        document_id, 
        epoch, 
        &fake_window_state, 
        &mut renderer_resources
    );

    let mut ops = Vec::new(); 
    layout_result_to_ops(document, &layout, &mut ops);
    Ok(vec![PdfPage::new(config.page_width, config.page_height, ops)])
}

fn fixup_xml(s: &str) -> String {
    let s = if !s.contains("<body>") { format!("<body>{s}</body>") } else { s.trim().to_string() };
    let s = if !s.contains("<html>") { format!("<html>{s}</html>") } else { s.trim().to_string() };
    s.trim().to_string()
}

fn fixup_xml_nodes(
    nodes: &[XmlNode]
) -> Vec<XmlNode> {
    // TODO!
    nodes.to_vec()
}

fn layout_result_to_ops(
    doc: &mut PdfDocument,
    lr: &LayoutResult,
    ops: &mut Vec<Op>,
) {

    /*
        Text {
            glyphs: Vec<GlyphInstance>,
            font_instance_key: FontInstanceKey,
            color: ColorU,
            glyph_options: Option<GlyphOptions>,
            overflow: (bool, bool),
            text_shadow: Option<StyleBoxShadow>,
        },
        Background {
            content: RectBackground,
            size: Option<StyleBackgroundSize>,
            offset: Option<StyleBackgroundPosition>,
            repeat: Option<StyleBackgroundRepeat>,
        },
        Border {
            widths: StyleBorderWidths,
            colors: StyleBorderColors,
            styles: StyleBorderStyles,
        },
        Image {
            size: LogicalSize,
            offset: LogicalPosition,
            image_rendering: ImageRendering,
            alpha_type: AlphaType,
            image_key: ImageKey,
            background_color: ColorU,
        },
    */

}

fn solve_layout(
    styled_dom: StyledDom,
    document_id: DocumentId,
    epoch: Epoch,
    fake_window_state: &FullWindowState,
    renderer_resources: &mut RendererResources
) -> LayoutResult {

    let fc_cache = azulc_lib::font_loading::build_font_cache();
    let image_cache = ImageCache::default();
    let callbacks = RenderCallbacks {
        insert_into_active_gl_textures_fn: azul_core::gl::insert_into_active_gl_textures,
        layout_fn: azul_layout::do_the_layout,
        load_font_fn: azulc_lib::font_loading::font_source_get_bytes, // needs feature="font_loading"
        parse_font_fn: azul_layout::parse_font_fn, // needs feature="text_layout"
    };

    // Solve the layout (the extra parameters are necessary because of IFrame recursion)
    let mut resource_updates = Vec::new();
    let mut solved_layout = SolvedLayout::new(
        styled_dom,
        epoch,
        &document_id,
        &fake_window_state,
        &mut resource_updates,
        IdNamespace(0),
        &image_cache,
        &fc_cache,
        &callbacks,
        renderer_resources,
        DpiScaleFactor { inner: FloatValue::const_new(1) },
    );

    solved_layout.layout_results.remove(0)
}

pub fn font_source_get_bytes(font_family: &azul_css::StyleFontFamily, fc_cache: &rust_fontconfig::FcFontCache) -> Option<azul_core::app_resources::LoadedFontSource> {
    println!("font source get bytes: {font_family:?}");
    println!("font source get bytes: {} fonts", fc_cache.list().len());
    None
}