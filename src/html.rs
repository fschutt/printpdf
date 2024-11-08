use std::collections::BTreeMap;
use azul_core::{app_resources::{DpiScaleFactor, Epoch, IdNamespace, ImageCache, ImageRef, ImageRefHash, RawImage, RendererResources, ResolvedImage}, callbacks::DocumentId, display_list::{DisplayListMsg, LayoutRectContent, RectBackground, RenderCallbacks, SolvedLayout, StyleBorderColors, StyleBorderRadius, StyleBorderStyles, StyleBorderWidths}, dom::{NodeData, NodeId}, styled_dom::{ContentGroup, DomId, StyledDom, StyledNode}, ui_solver::{LayoutResult, PositionedRectangle}, window::{FullWindowState, LogicalSize}, xml::{find_node_by_type, get_body_node, XmlComponentMap, XmlNode}};
use azul_css::{CssPropertyValue, FloatValue, LayoutDisplay, StyleTextColor};
use base64::Engine;
use rust_fontconfig::{FcFont, FcFontCache, FcPattern};
use crate::{BuiltinFont, Mm, Op, PdfDocument, PdfPage, Pt};

const DPI_SCALE: DpiScaleFactor = DpiScaleFactor { inner: FloatValue::const_new(1) };
const ID_NAMESPACE: IdNamespace = IdNamespace(0);
const EPOCH: Epoch = Epoch::new();
const DOCUMENT_ID: DocumentId = DocumentId {
    namespace_id: ID_NAMESPACE,
    id: 0,
};

pub type Base64String = String;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct XmlRenderOptions {
    pub images: BTreeMap<String, Base64String>,
    pub fonts: BTreeMap<String, Base64String>,
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

    let size = LogicalSize {
        width: config.page_width.into_pt().0,
        height: config.page_height.into_pt().0,
    };

    let root_nodes = azulc_lib::xml::parse_xml_string(&fixup_xml(file_contents))
    .map_err(|e| format!("Error parsing XML: {}", e.to_string()))?;

    let fixup = fixup_xml_nodes(&root_nodes);

    let styled_dom = azul_core::xml::str_to_dom(
        fixup.as_ref(), 
        &mut XmlComponentMap::default()
    )
    .map_err(|e| format!("Error constructing DOM: {}", e.to_string()))?;

    let dom_id = DomId { inner: 0 };
    let mut fake_window_state = FullWindowState::default();
    fake_window_state.size.dimensions = size;
    let mut renderer_resources = RendererResources::default();
    let image_cache = ImageCache {
        image_id_map: config.images.iter().filter_map(|(id, bytes)| {
            let bytes = base64::prelude::BASE64_STANDARD.decode(bytes).ok()?;
            let decoded = crate::image::RawImage::decode_from_bytes(&bytes).ok()?;
            let raw_image = crate::image::translate_to_internal_rawimage(&decoded);
            Some((id.clone().into(), ImageRef::new_rawimage(raw_image)?))
        }).collect()
    };

    let new_image_keys = styled_dom.scan_for_image_keys(&image_cache);
    let fonts_in_dom = styled_dom.scan_for_font_keys(&renderer_resources);

    // let builtin_fonts = get_used_builtin_fonts ;
    let mut fc_cache = FcFontCache::default();
    fc_cache
    .with_memory_fonts(&get_system_fonts())
    .with_memory_fonts(&[
        get_fcpat(BuiltinFont::TimesRoman),
        get_fcpat(BuiltinFont::TimesBold),
        get_fcpat(BuiltinFont::TimesItalic),
        get_fcpat(BuiltinFont::TimesBoldItalic),
        get_fcpat(BuiltinFont::Helvetica),
        get_fcpat(BuiltinFont::HelveticaBold),
        get_fcpat(BuiltinFont::HelveticaOblique),
        get_fcpat(BuiltinFont::HelveticaBoldOblique),
        get_fcpat(BuiltinFont::Courier),
        get_fcpat(BuiltinFont::CourierOblique),
        get_fcpat(BuiltinFont::CourierBold),
        get_fcpat(BuiltinFont::CourierBoldOblique),
        get_fcpat(BuiltinFont::Symbol),
        get_fcpat(BuiltinFont::ZapfDingbats),
    ])
    .with_memory_fonts(&config.fonts
        .iter()
        .filter_map(|(id, font_base64)| {
            let bytes = base64::prelude::BASE64_STANDARD.decode(font_base64).ok()?;
            let pat = FcPattern {
                name: Some(id.split(".").next().unwrap_or("").to_string()),
                .. Default::default()
            };
            let font = FcFont {
                bytes: bytes,
                font_index: 0,
            };
            Some((pat, font))
        }).collect::<Vec<_>>()
    );

    let add_font_resource_updates = azul_core::app_resources::build_add_font_resource_updates(
        &mut renderer_resources,
        DPI_SCALE,
        &fc_cache,
        ID_NAMESPACE,
        &fonts_in_dom,
        azulc_lib::font_loading::font_source_get_bytes,
        azul_text_layout::parse_font_fn,
    );

    let add_image_resource_updates = azul_core::app_resources::build_add_image_resource_updates(
        &renderer_resources,
        ID_NAMESPACE,
        EPOCH,
        &DOCUMENT_ID,
        &new_image_keys,
        azul_core::gl::insert_into_active_gl_textures,
    );

    let mut updates = Vec::new();
    azul_core::app_resources::add_resources(
        &mut renderer_resources,
        &mut updates,
        add_font_resource_updates,
        add_image_resource_updates,
    );

    let layout = solve_layout(
        styled_dom, 
        DOCUMENT_ID, 
        EPOCH, 
        &fake_window_state, 
        &mut renderer_resources
    );

    let mut ops = Vec::new(); 
    layout_result_to_ops(document, &layout, &renderer_resources, &mut ops, config.page_height.into_pt());
    Ok(vec![PdfPage::new(config.page_width, config.page_height, ops)])
}

fn get_system_fonts() -> Vec<(FcPattern, FcFont)> {
    let f = vec![
        ("serif", BuiltinFont::TimesRoman),
        ("sans-serif", BuiltinFont::Helvetica),
        ("cursive", BuiltinFont::TimesItalic),
        ("fantasy", BuiltinFont::TimesItalic),
        ("monospace", BuiltinFont::Courier),
    ];
    f.iter().map(|(id, f)| {
        let subset_font = f.get_subset_font();
        (FcPattern {
            name: Some(id.to_string()),
            .. Default::default()
        }, FcFont {
            bytes: subset_font.bytes.clone(),
            font_index: 0,
        })
    }).collect()
}

fn get_fcpat(b: BuiltinFont) -> (FcPattern, FcFont) {
    let subset_font = b.get_subset_font();
    (FcPattern {
        name: Some(b.get_id().to_string()),
        .. Default::default()
    }, FcFont {
        bytes: subset_font.bytes.clone(),
        font_index: 0,
    })
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
    layout_result: &LayoutResult,
    renderer_resources: &RendererResources,
    ops: &mut Vec<Op>,
    page_height: Pt,
) {

    let rects_in_rendering_order = layout_result.styled_dom.get_rects_in_rendering_order();
    
    // TODO: break layoutresult into pages
    // let root_width = layout_result.width_calculated_rects.as_ref()[NodeId::ZERO].overflow_width();
    // let root_height = layout_result.height_calculated_rects.as_ref()[NodeId::ZERO].overflow_height();
    // let root_size = LogicalSize::new(root_width, root_height);

    let _ = displaylist_handle_rect(
        doc,
        ops,
        layout_result,
        renderer_resources,
        rects_in_rendering_order.root.into_crate_internal().unwrap(),
        page_height,
    );

    for c in rects_in_rendering_order.children.as_slice() {
        push_rectangles_into_displaylist(doc, ops, layout_result, renderer_resources, c, page_height);
    }

}

fn push_rectangles_into_displaylist<'a>(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    layout_result: &LayoutResult,
    renderer_resources: &'a RendererResources,
    root_content_group: &ContentGroup,
    page_height: Pt,
) -> Option<()> {

    displaylist_handle_rect(
        doc, 
        ops,
        layout_result,
        renderer_resources,
        root_content_group.root.into_crate_internal().unwrap(),
        page_height,
    )?;

    for c in root_content_group.children.iter() {
        push_rectangles_into_displaylist(doc, ops, layout_result, renderer_resources, c, page_height);
    }
    
    Some(())
}

fn displaylist_handle_rect(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    layout_result: &LayoutResult,
    renderer_resources: &RendererResources,
    rect_idx: NodeId,
    page_height: Pt,
) -> Option<()> {

    use crate::units::Pt;

    let mut newops = Vec::new();

    let styled_node = &layout_result.styled_dom.styled_nodes.as_container()[rect_idx];
    let html_node = &layout_result.styled_dom.node_data.as_container()[rect_idx];

    if is_display_none(layout_result, html_node, rect_idx, styled_node) {
        return None;
    }

    let positioned_rect = &layout_result.rects.as_ref()[rect_idx];
    let border_radius = get_border_radius(layout_result, html_node, rect_idx, styled_node);
    let background_content = get_background_content(layout_result, html_node, rect_idx, styled_node);
    let opt_border = get_opt_border(layout_result, html_node, rect_idx, styled_node); 
    let opt_image = get_image_node(html_node, renderer_resources);
    let opt_text = get_text_node(layout_result, rect_idx, html_node, styled_node);

    for b in background_content.iter() {
        match &b.content {
            RectBackground::Color(c) => {
                let staticoffset = positioned_rect.position.get_static_offset();
                let rect = crate::graphics::Rect {
                    x: Pt(staticoffset.x),
                    y: Pt(page_height.0 - staticoffset.y),
                    width: Pt(positioned_rect.size.width),
                    height: Pt(positioned_rect.size.height),
                };
                newops.push(Op::SetFillColor { 
                    col: crate::Color::Rgb(crate::Rgb { 
                        r: c.r as f32 / 255.0, 
                        g: c.g as f32 / 255.0, 
                        b: c.b as f32 / 255.0, 
                        icc_profile: None 
                    }) 
                });
                newops.push(Op::DrawPolygon { polygon: rect.to_polygon() });
            },
            _ => { }
        }
    }

    if let Some(border) = opt_border.as_ref() {

        let (color_top, color_right, color_bottom, color_left) = (
            border.colors.top.and_then(|ct| ct.get_property_or_default()).unwrap_or_default(),
            border.colors.right.and_then(|cr| cr.get_property_or_default()).unwrap_or_default(),
            border.colors.bottom.and_then(|cb| cb.get_property_or_default()).unwrap_or_default(),
            border.colors.left.and_then(|cl| cl.get_property_or_default()).unwrap_or_default(),
        );

        let (width_top, width_right, width_bottom, width_left) = (
            border.widths.top.map(|w| w.map_property(|w| w.inner)).and_then(CssPropertyValue::get_property_or_default).unwrap_or_default(),
            border.widths.right.map(|w| w.map_property(|w| w.inner)).and_then(CssPropertyValue::get_property_or_default).unwrap_or_default(),
            border.widths.bottom.map(|w| w.map_property(|w| w.inner)).and_then(CssPropertyValue::get_property_or_default).unwrap_or_default(),
            border.widths.left.map(|w| w.map_property(|w| w.inner)).and_then(CssPropertyValue::get_property_or_default).unwrap_or_default(),
        );

        let staticoffset = positioned_rect.position.get_static_offset();
        let rect = crate::graphics::Rect {
            x: Pt(staticoffset.x),
            y: Pt(page_height.0 - staticoffset.y),
            width: Pt(positioned_rect.size.width),
            height: Pt(positioned_rect.size.height),
        };

        newops.push(Op::SetOutlineThickness { 
            pt: Pt(width_top.to_pixels(positioned_rect.size.height)) 
        });
        newops.push(Op::SetOutlineColor { 
            col: crate::Color::Rgb(crate::Rgb { 
                r: color_top.inner.r as f32 / 255.0, 
                g: color_top.inner.g as f32 / 255.0, 
                b: color_top.inner.b as f32 / 255.0, 
                icc_profile: None 
            }) 
        });
        newops.push(Op::DrawLine { line: rect.to_line() });
    }

    if !newops.is_empty() {
        ops.push(Op::SaveGraphicsState);
        ops.append(&mut newops);
        ops.push(Op::RestoreGraphicsState);
    }

    Some(())
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
        ID_NAMESPACE,
        &image_cache,
        &fc_cache,
        &callbacks,
        renderer_resources,
        DPI_SCALE,
    );

    solved_layout.layout_results.remove(0)
}

// test if an item is set to display:none
fn is_display_none(
    layout_result: &LayoutResult, 
    html_node: &NodeData, 
    rect_idx: NodeId, 
    styled_node: &StyledNode
) -> bool {
    let display = layout_result
    .styled_dom
    .get_css_property_cache()
    .get_display(&html_node, &rect_idx, &styled_node.state)
    .cloned()
    .unwrap_or_default();

    display == CssPropertyValue::None || 
    display == CssPropertyValue::Exact(LayoutDisplay::None)
}

fn get_border_radius(
    layout_result: &LayoutResult, 
    html_node: &NodeData, 
    rect_idx: NodeId, 
    styled_node: &StyledNode
) -> StyleBorderRadius {
    StyleBorderRadius {
        top_left: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_top_left_radius(&html_node, &rect_idx, &styled_node.state)
            .cloned(),
        top_right: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_top_right_radius(&html_node, &rect_idx, &styled_node.state)
            .cloned(),
        bottom_left: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_bottom_left_radius(&html_node, &rect_idx, &styled_node.state)
            .cloned(),
        bottom_right: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_bottom_right_radius(&html_node, &rect_idx, &styled_node.state)
            .cloned(),
    }
}

#[derive(Debug)]
struct LayoutRectContentBackground {
    content: azul_core::display_list::RectBackground,
    size: Option<azul_css::StyleBackgroundSize>,
    offset: Option<azul_css::StyleBackgroundPosition>,
    repeat: Option<azul_css::StyleBackgroundRepeat>,
}

fn get_background_content(
    layout_result: &LayoutResult, 
    html_node: &NodeData, 
    rect_idx: NodeId, 
    styled_node: &StyledNode
) -> Vec<LayoutRectContentBackground> {
    use azul_css::{
        StyleBackgroundPositionVec, 
        StyleBackgroundRepeatVec, 
        StyleBackgroundSizeVec,
    };

    let bg_opt = layout_result
    .styled_dom
    .get_css_property_cache()
    .get_background_content(&html_node, &rect_idx, &styled_node.state);

    let mut v = Vec::new();

    if let Some(bg) = bg_opt.as_ref().and_then(|br| br.get_property()) {

        let default_bg_size_vec: StyleBackgroundSizeVec = Vec::new().into();
        let default_bg_position_vec: StyleBackgroundPositionVec = Vec::new().into();
        let default_bg_repeat_vec: StyleBackgroundRepeatVec = Vec::new().into();

        let bg_sizes_opt = layout_result
            .styled_dom
            .get_css_property_cache()
            .get_background_size(&html_node, &rect_idx, &styled_node.state);
        let bg_positions_opt = layout_result
            .styled_dom
            .get_css_property_cache()
            .get_background_position(&html_node, &rect_idx, &styled_node.state);
        let bg_repeats_opt = layout_result
            .styled_dom
            .get_css_property_cache()
            .get_background_repeat(&html_node, &rect_idx, &styled_node.state);

        let bg_sizes = bg_sizes_opt
            .as_ref()
            .and_then(|p| p.get_property())
            .unwrap_or(&default_bg_size_vec);
        let bg_positions = bg_positions_opt
            .as_ref()
            .and_then(|p| p.get_property())
            .unwrap_or(&default_bg_position_vec);
        let bg_repeats = bg_repeats_opt
            .as_ref()
            .and_then(|p| p.get_property())
            .unwrap_or(&default_bg_repeat_vec);

        for (bg_index, bg) in bg.iter().enumerate() {
            use azul_css::StyleBackgroundContent::*;

            let background_content = match bg {
                LinearGradient(lg) => Some(RectBackground::LinearGradient(lg.clone())),
                RadialGradient(rg) => Some(RectBackground::RadialGradient(rg.clone())),
                ConicGradient(cg) => Some(RectBackground::ConicGradient(cg.clone())),
                Image(_) => None, // TODO
                Color(c) => Some(RectBackground::Color(*c)),
            };

            let bg_size = bg_sizes.get(bg_index).or(bg_sizes.get(0)).copied();
            let bg_position = bg_positions.get(bg_index).or(bg_positions.get(0)).copied();
            let bg_repeat = bg_repeats.get(bg_index).or(bg_repeats.get(0)).copied();

            if let Some(background_content) = background_content {
                v.push(LayoutRectContentBackground {
                    content: background_content,
                    size: bg_size.clone(),
                    offset: bg_position.clone(),
                    repeat: bg_repeat.clone(),
                });
            }
        }
    }

    v
}

fn get_image_node<'a>(
    html_node: &NodeData, 
    renderer_resources: &'a RendererResources
) -> Option<(&'a ResolvedImage, ImageRefHash, LogicalSize)> {
    
    use azul_core::dom::NodeType;
    use azul_core::app_resources::DecodedImage;

    let image_ref = match html_node.get_node_type() {
        NodeType::Image(image_ref) => image_ref,
        _ => return None,
    };
    
    let image_hash = image_ref.get_hash();
    let image_size = image_ref.get_size();

    let i = match image_ref.get_data() {
        DecodedImage::Raw(_) => renderer_resources.get_image(&image_hash),
        _ => None,
    }?;

    Some((i, image_hash, image_size))
}

fn get_text_node(
    layout_result: &LayoutResult,
    rect_idx: NodeId,
    html_node: &NodeData,
    styled_node: &StyledNode,
) -> Option<(azul_core::callbacks::InlineText, StyleTextColor)> {

    if !html_node.is_text_node() {
        return None;
    }

    let rects = layout_result.rects.as_ref();
    let words = layout_result.words_cache.get(&rect_idx)?;
    let shaped_words = layout_result.shaped_words_cache.get(&rect_idx)?;
    let word_positions = layout_result.positioned_words_cache.get(&rect_idx)?;
    let positioned_rect = rects.get(rect_idx)?;
    let (_, inline_text_layout) = positioned_rect.resolved_text_layout_options.as_ref()?;
    let inline_text = azul_core::app_resources::get_inline_text(
        &words,
        &shaped_words,
        &word_positions.0,
        &inline_text_layout,
    );
    let text_color = layout_result
        .styled_dom
        .get_css_property_cache()
        .get_text_color_or_default(&html_node, &rect_idx, &styled_node.state);

    Some((inline_text, text_color))
}

#[derive(Debug)]
struct LayoutRectContentBorder {
    widths: StyleBorderWidths,
    colors: StyleBorderColors,
    styles: StyleBorderStyles,
}

fn get_opt_border(
    layout_result: &LayoutResult, 
    html_node: &NodeData, 
    rect_idx: NodeId, 
    styled_node: &StyledNode,
) -> Option<LayoutRectContentBorder> {

    if !layout_result
    .styled_dom
    .get_css_property_cache()
    .has_border(&html_node, &rect_idx, &styled_node.state) {
        return None;
    }

    Some(LayoutRectContentBorder {
        widths: StyleBorderWidths {
            top: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_top_width(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            left: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_left_width(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            bottom: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_bottom_width(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            right: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_right_width(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
        },
        colors: StyleBorderColors {
            top: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_top_color(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            left: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_left_color(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            bottom: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_bottom_color(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            right: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_right_color(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
        },
        styles: StyleBorderStyles {
            top: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_top_style(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            left: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_left_style(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            bottom: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_bottom_style(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
            right: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_right_style(&html_node, &rect_idx, &styled_node.state)
                .cloned(),
        },
    })
}