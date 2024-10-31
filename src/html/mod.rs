use azul_core::{app_resources::{DpiScaleFactor, Epoch, IdNamespace, ImageCache, RendererResources}, callbacks::DocumentId, display_list::{DisplayListMsg, LayoutRectContent, RenderCallbacks, SolvedLayout}, styled_dom::{DomId, StyledDom}, ui_solver::LayoutResult, window::{FullWindowState, LogicalSize}, xml::XmlComponentMap};
use azul_css::FloatValue;
use crate::{Mm, Op, PdfPage};

pub fn xml_to_pages(file_contents: &str, page_width: Mm, page_height: Mm) -> Result<Vec<PdfPage>, String> {

    let size = LogicalSize::new(page_width.into_pt().0, page_height.into_pt().0);

    let root_nodes = azulc_lib::xml::parse_xml_string(&file_contents.trim())
    .map_err(|e| format!("Error parsing XML: {}", e.to_string()))?;

    let styled_dom = azul_core::xml::str_to_dom(root_nodes.as_ref(), &mut XmlComponentMap::default())
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

    let display_list = LayoutResult::get_cached_display_list(
        &document_id,
        dom_id,
        epoch,
        &[layout],
        &fake_window_state,
        &azul_core::app_resources::GlTextureCache::default(),
        &renderer_resources,
        &image_cache,
    );

    let mut ops = Vec::new(); 
    display_list_to_ops(&display_list.root, &mut ops);
    Ok(vec![PdfPage::new(page_width, page_height, ops)])
}

fn display_list_to_ops(
    msg: &DisplayListMsg,
    ops: &mut Vec<Op>,
) {

    let f = match msg {
        DisplayListMsg::Frame(f) => f,
        DisplayListMsg::ScrollFrame(sf) => &sf.frame,
        DisplayListMsg::IFrame(_, _, _, _) => { return; },
    };
    
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

    for c in f.children.iter() {
        display_list_to_ops(c, ops);
    }
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