use std::{collections::BTreeMap, str::FromStr};

use azul_core::{
    app_resources::{
        DecodedImage, DpiScaleFactor, Epoch, IdNamespace, ImageCache, ImageRef, RendererResources,
    },
    callbacks::{DocumentId, InlineText, InlineWord},
    display_list::{
        RectBackground, RenderCallbacks, SolvedLayout, StyleBorderColors, StyleBorderRadius,
        StyleBorderStyles, StyleBorderWidths,
    },
    dom::{NodeData, NodeId},
    pagination::{PaginatedNode, PaginatedPage},
    styled_dom::StyledNode,
    ui_solver::LayoutResult,
    window::{AzStringPair, FullWindowState, LogicalSize},
};
pub use azul_core::{
    dom::Dom,
    styled_dom::StyledDom,
    xml::{
        CompileError, ComponentArguments, ComponentParseError, DynamicXmlComponent,
        FilteredComponentArguments, RenderDomError, XmlComponent, XmlComponentMap,
        XmlComponentTrait, XmlNode, XmlTextContent,
    },
};
pub use azul_css::parser::CssApiWrapper;
use azul_css::{CssPropertyValue, FloatValue, LayoutDisplay, StyleTextColor};
use kuchiki::{traits::*, NodeRef};
use rust_fontconfig::{FcFont, FcFontCache, FcPattern, PatternMatch};
use serde_derive::{Deserialize, Serialize};
use svg2pdf::usvg::tiny_skia_path::Scalar;

use crate::{
    components::ImageInfo, Base64OrRaw, BuiltinFont, Color, FontId, GeneratePdfOptions, Mm, Op,
    PdfDocument, PdfPage, PdfResources, PdfWarnMsg, Pt, TextItem, TextMatrix,
};

const DPI_SCALE: DpiScaleFactor = DpiScaleFactor {
    inner: FloatValue::const_new(1),
};
const ID_NAMESPACE: IdNamespace = IdNamespace(0);
const EPOCH: Epoch = Epoch::new();
const DOCUMENT_ID: DocumentId = DocumentId {
    namespace_id: ID_NAMESPACE,
    id: 0,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlRenderOptions {
    #[serde(default)]
    pub images: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub fonts: BTreeMap<String, Vec<u8>>,
    #[serde(default = "default_page_width")]
    pub page_width: Mm,
    #[serde(default = "default_page_height")]
    pub page_height: Mm,
    #[serde(default, skip)]
    pub components: Vec<XmlComponent>,
}

// PartialEq implementation
impl PartialEq for XmlRenderOptions {
    fn eq(&self, other: &Self) -> bool {
        self.images == other.images
            && self.fonts == other.fonts
            && self.page_width == other.page_width
            && self.page_height == other.page_height
            && self.components.len() == other.components.len()
    }
}

// PartialOrd implementation
impl PartialOrd for XmlRenderOptions {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.images.partial_cmp(&other.images) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        match self.fonts.partial_cmp(&other.fonts) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        match self.page_width.partial_cmp(&other.page_width) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        match self.page_height.partial_cmp(&other.page_height) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        Some(std::cmp::Ordering::Equal)
    }
}

fn default_page_width() -> Mm {
    Mm(210.0)
}
fn default_page_height() -> Mm {
    Mm(297.0)
}

impl Default for XmlRenderOptions {
    fn default() -> Self {
        Self {
            images: Default::default(),
            fonts: Default::default(),
            page_width: default_page_width(),
            page_height: default_page_height(),
            components: Default::default(),
        }
    }
}

pub(crate) async fn xml_to_pages_async(
    file_contents: &str,
    config: XmlRenderOptions,
    document: &mut PdfDocument,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<PdfPage>, String> {
    let mut image_id_map = BTreeMap::new();

    for (id, bytes) in config.images.iter() {
        let decoded = match crate::image::RawImage::decode_from_bytes_async(&bytes, warnings)
            .await
            .ok()
        {
            Some(s) => s,
            None => continue,
        };
        let raw_image = crate::image::translate_to_internal_rawimage(&decoded);
        let ir = match ImageRef::new_rawimage(raw_image) {
            Some(s) => s,
            None => continue,
        };
        image_id_map.insert(id.clone().into(), ir);
    }

    xml_to_pages_inner(
        file_contents,
        config,
        document,
        ImageCache { image_id_map },
        warnings,
    )
}

pub(crate) fn html_to_document_inner(
    html: &str,
    images: &BTreeMap<String, Base64OrRaw>,
    fonts: &BTreeMap<String, Base64OrRaw>,
    options: &GeneratePdfOptions,
    _warnings: &mut Vec<PdfWarnMsg>,
) -> Result<(String, PdfDocument, crate::XmlRenderOptions), String> {
    // Transform HTML to XML with extracted configuration
    let (transformed_xml, config) = crate::html::process_html_for_rendering(html);

    // Create document with title from input or extracted from HTML
    let title = config.title.clone().unwrap_or_default();

    let mut pdf = crate::PdfDocument::new(&title);

    // Prepare rendering options
    let mut opts = crate::html::XmlRenderOptions {
        page_width: Mm(options.page_width.unwrap_or(210.0)),
        page_height: Mm(options.page_height.unwrap_or(297.0)),
        images: images
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.decode_bytes().ok()?)))
            .collect(),
        fonts: fonts
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

pub(crate) fn xml_to_pages(
    file_contents: &str,
    config: XmlRenderOptions,
    document: &mut PdfDocument,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<PdfPage>, String> {
    let image_cache = ImageCache {
        image_id_map: config
            .images
            .iter()
            .filter_map(|(id, bytes)| {
                // let bytes = base64::prelude::BASE64_STANDARD.decode(bytes).ok()?;
                let decoded = crate::image::RawImage::decode_from_bytes(&bytes, warnings).ok()?;
                let raw_image = crate::image::translate_to_internal_rawimage(&decoded);
                Some((id.clone().into(), ImageRef::new_rawimage(raw_image)?))
            })
            .collect(),
    };

    xml_to_pages_inner(file_contents, config, document, image_cache, warnings)
}

fn get_fc_cache(fonts: &BTreeMap<String, Vec<u8>>) -> FcFontCache {
    let mut fc_cache = FcFontCache::default();
    fc_cache
        .with_memory_fonts(get_system_fonts())
        .with_memory_fonts(vec![
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
        .with_memory_fonts(
            fonts
                .iter()
                .filter_map(|(id, bytes)| {
                    // let bytes = base64::prelude::BASE64_STANDARD.decode(font_base64).ok()?;
                    let pat = FcPattern {
                        name: Some(id.split(".").next().unwrap_or("").to_string()),
                        ..Default::default()
                    };
                    let font = FcFont {
                        id: id.to_string(),
                        bytes: bytes.clone(),
                        font_index: 0,
                    };
                    Some((pat, font))
                })
                .collect::<Vec<_>>(),
        );
    fc_cache
}

#[test]
fn test_default_font() {
    let fc_cache = get_fc_cache(&BTreeMap::new());
    let mut msg = Vec::new();
    println!(
        "default font: {:?}",
        fc_cache.query(&FcPattern::default(), &mut msg)
    );
    println!("{msg:#?}");
}

fn xml_to_pages_inner(
    file_contents: &str,
    config: XmlRenderOptions,
    document: &mut PdfDocument,
    image_cache: ImageCache,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<PdfPage>, String> {
    let size = LogicalSize {
        width: config.page_width.into_pt().0,
        height: config.page_height.into_pt().0,
    };

    // inserts images into the PDF resources and changes the src="..."
    let xml = fixup_xml(file_contents, document, &config, warnings);

    let root_nodes = azul_layout::xml::parse_xml_string(&xml)
        .map_err(|e| format!("Error parsing XML: {}", e))?;

    let fixup = fixup_xml_nodes(&root_nodes);

    let mut components = crate::components::printpdf_default_components();
    for c in config.components {
        components.register_component(c);
    }

    let styled_dom = azul_core::xml::str_to_dom(
        fixup.as_ref(),
        &mut components,
        Some(config.page_width.into_pt().0),
    )
    .map_err(|e| format!("Error constructing DOM: {}", e.to_string()))?;

    let mut fake_window_state = FullWindowState::default();
    fake_window_state.size.dimensions = size;
    let mut renderer_resources = RendererResources::default();

    let new_image_keys = styled_dom.scan_for_image_keys(&image_cache);
    let fonts_in_dom = styled_dom.scan_for_font_keys(&renderer_resources);

    let fc_cache = get_fc_cache(&config.fonts);

    let add_font_resource_updates = azul_core::app_resources::build_add_font_resource_updates(
        &mut renderer_resources,
        DPI_SCALE,
        &fc_cache,
        ID_NAMESPACE,
        &fonts_in_dom,
        azul_layout::font::loading::font_source_get_bytes,
        azul_layout::parse_font_fn,
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
        &mut renderer_resources,
    );

    // Break layout into pages using the pagination module
    let paginated_pages = azul_core::pagination::paginate_layout_result(
        &layout.styled_dom.node_hierarchy.as_container(),
        &layout.rects.as_ref(),
        config.page_height.into_pt().0,
    );

    let pages = paginated_pages
        .into_iter()
        .map(|pp| {
            let mut ops = Vec::new();
            layout_result_to_ops(
                document,
                &mut ops,
                &layout,
                &pp,
                &renderer_resources,
                config.page_height.into_pt(),
                warnings,
            );

            PdfPage::new(config.page_width, config.page_height, ops)
        })
        .collect();

    Ok(pages)
}

fn get_system_fonts() -> Vec<(FcPattern, FcFont)> {
    let f = [
        ("serif", BuiltinFont::TimesRoman),
        ("sans-serif", BuiltinFont::Helvetica),
        ("cursive", BuiltinFont::TimesItalic),
        ("fantasy", BuiltinFont::TimesItalic),
        ("monospace", BuiltinFont::Courier),
    ];
    f.iter()
        .map(|(id, f)| {
            let subset_font = f.get_subset_font();
            (
                FcPattern {
                    name: Some(id.to_string()),
                    ..Default::default()
                },
                FcFont {
                    id: id.to_string(),
                    bytes: subset_font.bytes.clone(),
                    font_index: 0,
                },
            )
        })
        .collect()
}

fn get_fcpat(b: BuiltinFont) -> (FcPattern, FcFont) {
    let subset_font = b.get_subset_font();
    (
        FcPattern {
            name: Some(b.get_id().to_string()),
            family: Some(b.get_id().to_string()),
            italic: if b.get_font_style() == "italic" {
                PatternMatch::True
            } else {
                PatternMatch::DontCare
            },
            bold: if b.get_font_weight() == "bold" {
                PatternMatch::True
            } else {
                PatternMatch::DontCare
            },
            ..Default::default()
        },
        FcFont {
            id: b.get_id().to_string(),
            bytes: subset_font.bytes.clone(),
            font_index: 0,
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageTypeInfo {
    Image,
    Svg,
}

impl Default for ImageTypeInfo {
    fn default() -> Self {
        ImageTypeInfo::Image
    }
}

fn fixup_xml(
    s: &str,
    doc: &mut PdfDocument,
    config: &XmlRenderOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> String {
    let s = if !s.contains("<body") {
        format!("<body>{s}</body>")
    } else {
        s.trim().to_string()
    };
    let s = if !s.contains("<html") {
        format!("<html>{s}</html>")
    } else {
        s.trim().to_string()
    };

    let mut s = s.trim().to_string();

    for (k, image_bytes) in config.images.iter() {
        let opt_svg = std::str::from_utf8(&image_bytes)
            .ok()
            .and_then(|s| crate::Svg::parse(s, warnings).ok());

        let img_info = match opt_svg {
            Some(o) => {
                let width = o.width.map(|s| s.0).unwrap_or(0);
                let height = o.height.map(|s| s.0).unwrap_or(0);
                let image_xobject_id = doc.add_xobject(&o);
                ImageInfo {
                    original_id: k.clone(),
                    xobject_id: image_xobject_id.0,
                    image_type: ImageTypeInfo::Svg,
                    width,
                    height,
                }
            }
            None => {
                let raw_image =
                    match crate::image::RawImage::decode_from_bytes(&image_bytes, warnings) {
                        Ok(o) => o,
                        Err(_) => {
                            continue;
                        }
                    };

                let width = raw_image.width;
                let height = raw_image.height;
                let image_xobject_id = doc.add_image(&raw_image);
                ImageInfo {
                    original_id: k.clone(),
                    xobject_id: image_xobject_id.0,
                    image_type: ImageTypeInfo::Image,
                    width,
                    height,
                }
            }
        };

        let json = serde_json::to_string(&img_info).unwrap_or_default();

        s = s
            .replace(&format!("src='{k}'"), &format!("src='{json}'"))
            .replace(&format!("src=\"{k}\""), &format!("src='{json}'"));
    }

    s
}

fn fixup_xml_nodes(nodes: &[XmlNode]) -> Vec<XmlNode> {
    // TODO!
    nodes.to_vec()
}

fn layout_result_to_ops(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    layout_result: &LayoutResult,
    pp: &PaginatedPage,
    renderer_resources: &RendererResources,
    page_height: Pt,
    warnings: &mut Vec<PdfWarnMsg>,
) {
    // Check if the page has a root node
    if let Some(root_node) = &pp.root {
        // Process the root node and its children recursively
        process_paginated_node(
            doc,
            ops,
            layout_result,
            renderer_resources,
            root_node,
            page_height,
            warnings,
        );
    }
}

fn displaylist_handle_rect_paginated(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    layout_result: &LayoutResult,
    renderer_resources: &RendererResources,
    node: &PaginatedNode,
    page_height: Pt,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Option<()> {
    use crate::units::Pt;

    let mut newops = Vec::new();

    // Get the original node ID and the paginated rect
    let original_node_id = node.id;
    let rect = &node.rect;

    // Get node data from the original layout result
    let styled_node = &layout_result.styled_dom.styled_nodes.as_container()[original_node_id];
    let html_node = &layout_result.styled_dom.node_data.as_container()[original_node_id];

    // Skip display:none elements
    if is_display_none(layout_result, html_node, original_node_id, styled_node) {
        return None;
    }

    // The rest of the function is similar to the original, but uses the paginated rect
    let _border_radius = get_border_radius(layout_result, html_node, original_node_id, styled_node);
    let background_content =
        get_background_content(layout_result, html_node, original_node_id, styled_node);
    let opt_border = get_opt_border(layout_result, html_node, original_node_id, styled_node);
    let opt_image = get_image_node(html_node);
    let opt_text = get_text_node(
        layout_result,
        original_node_id,
        html_node,
        styled_node,
        renderer_resources,
        &mut doc.resources,
        warnings,
    );

    for b in background_content.iter() {
        if let RectBackground::Color(c) = &b.content {
            // PDF coordinates start at the bottom-left, but our rect has top-left origin
            // Convert Y coordinate to PDF space
            let rect_obj = crate::graphics::Rect {
                x: Pt(rect.origin.x),
                y: Pt(page_height.0 - rect.origin.y - rect.size.height), // Invert Y coordinate
                width: Pt(rect.size.width),
                height: Pt(rect.size.height),
            };

            newops.push(Op::SetFillColor {
                col: crate::Color::Rgb(crate::Rgb {
                    r: c.r as f32 / 255.0,
                    g: c.g as f32 / 255.0,
                    b: c.b as f32 / 255.0,
                    icc_profile: None,
                }),
            });
            newops.push(Op::DrawPolygon {
                polygon: rect_obj.to_polygon(),
            });
        }
    }

    if let Some(border) = opt_border.as_ref() {
        let (color_top, _color_right, _color_bottom, _color_left) = (
            border
                .colors
                .top
                .and_then(|ct| ct.get_property_or_default())
                .unwrap_or_default(),
            border
                .colors
                .right
                .and_then(|cr| cr.get_property_or_default())
                .unwrap_or_default(),
            border
                .colors
                .bottom
                .and_then(|cb| cb.get_property_or_default())
                .unwrap_or_default(),
            border
                .colors
                .left
                .and_then(|cl| cl.get_property_or_default())
                .unwrap_or_default(),
        );

        // TODO! proper layout of rectangles!
        let (width_top, _width_right, _width_bottom, _width_left) = (
            border
                .widths
                .top
                .map(|w| w.map_property(|w| w.inner))
                .and_then(CssPropertyValue::get_property_or_default)
                .unwrap_or_default(),
            border
                .widths
                .right
                .map(|w| w.map_property(|w| w.inner))
                .and_then(CssPropertyValue::get_property_or_default)
                .unwrap_or_default(),
            border
                .widths
                .bottom
                .map(|w| w.map_property(|w| w.inner))
                .and_then(CssPropertyValue::get_property_or_default)
                .unwrap_or_default(),
            border
                .widths
                .left
                .map(|w| w.map_property(|w| w.inner))
                .and_then(CssPropertyValue::get_property_or_default)
                .unwrap_or_default(),
        );

        let rect_obj = crate::graphics::Rect {
            x: Pt(rect.origin.x),
            y: Pt(page_height.0 - rect.origin.y - rect.size.height), // Invert Y coordinate
            width: Pt(rect.size.width),
            height: Pt(rect.size.height),
        };

        newops.push(Op::SetOutlineThickness {
            pt: Pt(width_top.to_pixels(rect.size.height)),
        });
        newops.push(Op::SetOutlineColor {
            col: crate::Color::Rgb(crate::Rgb {
                r: color_top.inner.r as f32 / 255.0,
                g: color_top.inner.g as f32 / 255.0,
                b: color_top.inner.b as f32 / 255.0,
                icc_profile: None,
            }),
        });
        newops.push(Op::DrawLine {
            line: rect_obj.to_line(),
        });
    }

    if let Some(image_info) = opt_image {
        let source_width = image_info.width;
        let source_height = image_info.width;
        let target_width = rect.size.width;
        let target_height = rect.size.height;

        let is_zero = target_width.is_nearly_zero()
            || target_height.is_nearly_zero()
            || source_height == 0
            || source_width == 0;

        if !is_zero {
            ops.push(Op::UseXobject {
                id: crate::XObjectId(image_info.xobject_id.clone()),
                transform: crate::XObjectTransform {
                    translate_x: Some(Pt(rect.origin.x)),
                    translate_y: Some(Pt(page_height.0 - rect.origin.y - rect.size.height)), // Invert Y coordinate for PDF
                    rotate: None,
                    scale_x: Some(target_width / source_width as f32),
                    scale_y: Some(target_height / source_height as f32),
                    dpi: None,
                },
            });
        }
    }

    if let Some((text, id, color, space_advance_scaled)) = opt_text {
        let mut text_ops = to_pdf_ops(&text, page_height, id, color, space_advance_scaled);
        ops.append(&mut text_ops);
    }

    if !newops.is_empty() {
        ops.push(Op::SaveGraphicsState);
        ops.append(&mut newops);
        ops.push(Op::RestoreGraphicsState);
    }

    Some(())
}

/// Converts the InlineText to a sequence of PDF operations for direct rendering
///
/// * `page_height` - The total height of the PDF page (needed for Y coordinate conversion)
/// * `font_id` - The PDF font identifier to use for rendering
/// * `text_color` - The text color to use
pub fn to_pdf_ops(
    it: &InlineText,
    page_height: Pt,
    font_id: FontId,
    text_color: StyleTextColor,
    space_advance_scaled: usize,
) -> Vec<Op> {
    let mut ops = Vec::new();

    // Start text section
    ops.push(Op::StartTextSection);

    // Set text color
    ops.push(Op::SetFillColor {
        col: Color::Rgb(crate::Rgb {
            r: text_color.inner.r as f32 / 255.0,
            g: text_color.inner.g as f32 / 255.0,
            b: text_color.inner.b as f32 / 255.0,
            icc_profile: None,
        }),
    });

    let is_builtin_font = BuiltinFont::from_id(&font_id.0);

    // Set font and size
    if let Some(bf) = is_builtin_font {
        ops.push(Op::SetFontSizeBuiltinFont {
            size: Pt(it.font_size_px),
            font: bf,
        });
    } else {
        ops.push(Op::SetFontSize {
            size: Pt(it.font_size_px),
            font: font_id.clone(),
        });
    }

    // Set line height
    ops.push(Op::SetLineHeight {
        lh: Pt(it.font_size_px),
    });

    // Process each line
    for (line_idx, line) in it.lines.iter().enumerate() {
        let line_origin = line.bounds.origin;

        // If not the first line, add a line break
        if line_idx > 0 {
            ops.push(Op::AddLineBreak);
        }

        // PDF coordinates: Y is from bottom, so we need to flip
        // Position at the baseline of this line
        let pdf_y = page_height.0 - (line_origin.y - it.baseline_descender_px);

        // Set text position for this line
        ops.push(Op::SetTextMatrix {
            matrix: TextMatrix::Translate(Pt(line_origin.x), Pt(pdf_y)),
        });

        // Process words in this line
        let mut text_items = Vec::new();
        let mut last_x_position = 0.0;

        for word in line.words.iter() {
            match word {
                InlineWord::Tab => {
                    let tab_width_in_spaces = 4.0;
                    let tab_x_advance = space_advance_scaled as f32 * tab_width_in_spaces;

                    if !text_items.is_empty() {
                        if let Some(bf) = is_builtin_font {
                            ops.push(Op::WriteTextBuiltinFont {
                                items: std::mem::take(&mut text_items),
                                font: bf,
                            });
                        } else {
                            ops.push(Op::WriteText {
                                items: std::mem::take(&mut text_items),
                                font: font_id.clone(),
                            });
                        }
                    }

                    ops.push(Op::SetTextMatrix {
                        matrix: TextMatrix::Translate(
                            Pt(last_x_position + tab_x_advance),
                            Pt(pdf_y),
                        ),
                    });
                    last_x_position += tab_x_advance;
                }
                InlineWord::Return => {
                    // Return is handled by line breaks, which we already process by iterating
                    // through lines
                    if !text_items.is_empty() {
                        if let Some(bf) = is_builtin_font {
                            ops.push(Op::WriteTextBuiltinFont {
                                items: std::mem::take(&mut text_items),
                                font: bf,
                            });
                        } else {
                            ops.push(Op::WriteText {
                                items: std::mem::take(&mut text_items),
                                font: font_id.clone(),
                            });
                        }
                    }
                }
                InlineWord::Space => {
                    // text_items.push(TextItem::Text(" ".to_string()));
                    last_x_position += space_advance_scaled as f32;
                }
                InlineWord::Word(text_contents) => {
                    let word_origin = text_contents.bounds.origin;

                    // If position changes significantly from expected, add positioning kerning
                    if !text_items.is_empty() && (word_origin.x - last_x_position).abs() > 0.1 {
                        // Add kerning offset
                        text_items.push(TextItem::Offset((last_x_position - word_origin.x) * -1.0));
                    }

                    // Process each glyph in the word
                    let mut word_text = String::new();

                    for glyph in text_contents.glyphs.iter() {
                        if let Some(ch) = glyph
                            .unicode_codepoint
                            .into_option()
                            .and_then(char::from_u32)
                        {
                            word_text.push(ch);
                        }
                    }

                    if !word_text.is_empty() {
                        text_items.push(TextItem::Text(word_text));
                    }

                    // Update last position
                    last_x_position = word_origin.x + text_contents.bounds.size.width;
                }
            }
        }

        // Write any remaining text for this line
        if !text_items.is_empty() {
            if let Some(bf) = is_builtin_font {
                ops.push(Op::WriteTextBuiltinFont {
                    items: text_items,
                    font: bf,
                });
            } else {
                ops.push(Op::WriteText {
                    items: text_items,
                    font: font_id.clone(),
                });
            }
        }
    }

    // End text section
    ops.push(Op::EndTextSection);

    ops
}

fn process_paginated_node(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    layout_result: &LayoutResult,
    renderer_resources: &RendererResources,
    node: &PaginatedNode,
    page_height: Pt,
    warnings: &mut Vec<PdfWarnMsg>,
) {
    // Process this node
    let _ = displaylist_handle_rect_paginated(
        doc,
        ops,
        layout_result,
        renderer_resources,
        node,
        page_height,
        warnings,
    );

    // Process its children recursively
    for child in &node.children {
        process_paginated_node(
            doc,
            ops,
            layout_result,
            renderer_resources,
            child,
            page_height,
            warnings,
        );
    }
}

fn solve_layout(
    styled_dom: StyledDom,
    document_id: DocumentId,
    epoch: Epoch,
    fake_window_state: &FullWindowState,
    renderer_resources: &mut RendererResources,
) -> LayoutResult {
    let fc_cache = azul_layout::font::loading::build_font_cache();
    let image_cache = ImageCache::default();
    let callbacks = RenderCallbacks {
        insert_into_active_gl_textures_fn: azul_core::gl::insert_into_active_gl_textures,
        layout_fn: azul_layout::solver2::do_the_layout,
        load_font_fn: azul_layout::font::loading::font_source_get_bytes,
        parse_font_fn: azul_layout::parse_font_fn,
    };

    // Solve the layout (the extra parameters are necessary because of IFrame recursion)
    let mut resource_updates = Vec::new();
    let mut debug = Some(Vec::new());
    println!("{}", styled_dom.get_html_string("", "", false));
    let mut solved_layout = SolvedLayout::new(
        styled_dom,
        epoch,
        &document_id,
        fake_window_state,
        &mut resource_updates,
        ID_NAMESPACE,
        &image_cache,
        &fc_cache,
        &callbacks,
        renderer_resources,
        DPI_SCALE,
        &mut debug,
    );

    solved_layout.layout_results.remove(0)
}

// test if an item is set to display:none
fn is_display_none(
    layout_result: &LayoutResult,
    html_node: &NodeData,
    rect_idx: NodeId,
    styled_node: &StyledNode,
) -> bool {
    let display = layout_result
        .styled_dom
        .get_css_property_cache()
        .get_display(html_node, &rect_idx, &styled_node.state)
        .cloned()
        .unwrap_or_default();

    display == CssPropertyValue::None || display == CssPropertyValue::Exact(LayoutDisplay::None)
}

fn get_border_radius(
    layout_result: &LayoutResult,
    html_node: &NodeData,
    rect_idx: NodeId,
    styled_node: &StyledNode,
) -> StyleBorderRadius {
    StyleBorderRadius {
        top_left: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_top_left_radius(html_node, &rect_idx, &styled_node.state)
            .cloned(),
        top_right: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_top_right_radius(html_node, &rect_idx, &styled_node.state)
            .cloned(),
        bottom_left: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_bottom_left_radius(html_node, &rect_idx, &styled_node.state)
            .cloned(),
        bottom_right: layout_result
            .styled_dom
            .get_css_property_cache()
            .get_border_bottom_right_radius(html_node, &rect_idx, &styled_node.state)
            .cloned(),
    }
}

#[derive(Debug)]
struct LayoutRectContentBackground {
    content: azul_core::display_list::RectBackground,
    #[allow(dead_code)]
    size: Option<azul_css::StyleBackgroundSize>,
    #[allow(dead_code)]
    offset: Option<azul_css::StyleBackgroundPosition>,
    #[allow(dead_code)]
    repeat: Option<azul_css::StyleBackgroundRepeat>,
}

fn get_background_content(
    layout_result: &LayoutResult,
    html_node: &NodeData,
    rect_idx: NodeId,
    styled_node: &StyledNode,
) -> Vec<LayoutRectContentBackground> {
    use azul_css::{StyleBackgroundPositionVec, StyleBackgroundRepeatVec, StyleBackgroundSizeVec};

    let bg_opt = layout_result
        .styled_dom
        .get_css_property_cache()
        .get_background_content(html_node, &rect_idx, &styled_node.state);

    let mut v = Vec::new();

    if let Some(bg) = bg_opt.as_ref().and_then(|br| br.get_property()) {
        let default_bg_size_vec: StyleBackgroundSizeVec = Vec::new().into();
        let default_bg_position_vec: StyleBackgroundPositionVec = Vec::new().into();
        let default_bg_repeat_vec: StyleBackgroundRepeatVec = Vec::new().into();

        let bg_sizes_opt = layout_result
            .styled_dom
            .get_css_property_cache()
            .get_background_size(html_node, &rect_idx, &styled_node.state);
        let bg_positions_opt = layout_result
            .styled_dom
            .get_css_property_cache()
            .get_background_position(html_node, &rect_idx, &styled_node.state);
        let bg_repeats_opt = layout_result
            .styled_dom
            .get_css_property_cache()
            .get_background_repeat(html_node, &rect_idx, &styled_node.state);

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
                    size: bg_size,
                    offset: bg_position,
                    repeat: bg_repeat,
                });
            }
        }
    }

    v
}

fn get_image_node(html_node: &NodeData) -> Option<ImageInfo> {
    use azul_core::dom::NodeType;

    let data = match html_node.get_node_type() {
        NodeType::Image(image_ref) => image_ref.get_data(),
        _ => return None,
    };

    if let DecodedImage::NullImage { tag, .. } = data {
        serde_json::from_slice(tag).ok()
    } else {
        None
    }
}

fn get_text_node(
    layout_result: &LayoutResult,
    rect_idx: NodeId,
    html_node: &NodeData,
    styled_node: &StyledNode,
    app_resources: &azul_core::app_resources::RendererResources,
    res: &mut PdfResources,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Option<(
    azul_core::callbacks::InlineText,
    crate::FontId,
    StyleTextColor,
    usize,
)> {
    use azul_core::styled_dom::StyleFontFamiliesHash;

    if !html_node.is_text_node() {
        return None;
    }

    let font_families = layout_result
        .styled_dom
        .get_css_property_cache()
        .get_font_id_or_default(html_node, &rect_idx, &styled_node.state);

    let sffh_1 = StyleFontFamiliesHash::new(font_families.as_slice());
    let sffh = app_resources.get_font_family(&sffh_1)?;
    let font_key = app_resources.get_font_key(sffh)?;
    let fd = app_resources.get_registered_font(font_key)?;
    let font_ref = &fd.0;

    let rects = layout_result.rects.as_ref();
    let words = layout_result.words_cache.get(&rect_idx)?;
    let shaped_words = layout_result.shaped_words_cache.get(&rect_idx)?;
    let word_positions = layout_result.positioned_words_cache.get(&rect_idx)?;
    let positioned_rect = rects.get(rect_idx)?;
    let (_, inline_text_layout) = positioned_rect.resolved_text_layout_options.as_ref()?;
    let inline_text = azul_core::app_resources::get_inline_text(
        words,
        shaped_words,
        &word_positions,
        inline_text_layout,
    );
    let text_color = layout_result
        .styled_dom
        .get_css_property_cache()
        .get_text_color_or_default(html_node, &rect_idx, &styled_node.state);

    // add font to resources if not existent
    let mut id = crate::FontId(format!("azul_font_family_{:032}", sffh.0));

    if !res.fonts.map.contains_key(&id) {
        // Check if builtin font
        if let Some(bf) = BuiltinFont::check_if_matches(&font_ref.get_data().bytes.as_slice()) {
            id = FontId(bf.get_id().to_string());
        } else {
            let font_bytes = font_ref.get_bytes();
            let parsed_font = crate::ParsedFont::from_bytes(font_bytes.as_slice(), 0, warnings)?;
            res.fonts.map.insert(id.clone(), parsed_font);
        }
    }

    Some((inline_text, id, text_color, shaped_words.space_advance))
}

#[derive(Debug)]
struct LayoutRectContentBorder {
    widths: StyleBorderWidths,
    colors: StyleBorderColors,
    #[allow(dead_code)]
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
        .has_border(html_node, &rect_idx, &styled_node.state)
    {
        return None;
    }

    Some(LayoutRectContentBorder {
        widths: StyleBorderWidths {
            top: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_top_width(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            left: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_left_width(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            bottom: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_bottom_width(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            right: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_right_width(html_node, &rect_idx, &styled_node.state)
                .cloned(),
        },
        colors: StyleBorderColors {
            top: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_top_color(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            left: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_left_color(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            bottom: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_bottom_color(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            right: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_right_color(html_node, &rect_idx, &styled_node.state)
                .cloned(),
        },
        styles: StyleBorderStyles {
            top: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_top_style(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            left: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_left_style(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            bottom: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_bottom_style(html_node, &rect_idx, &styled_node.state)
                .cloned(),
            right: layout_result
                .styled_dom
                .get_css_property_cache()
                .get_border_right_style(html_node, &rect_idx, &styled_node.state)
                .cloned(),
        },
    })
}

// --- HTML to XML transformation

/// Configuration options extracted from HTML meta tags
pub struct HtmlExtractedConfig {
    /// Title extracted from the <title> element
    pub title: Option<String>,
    /// PDF page width from meta tags
    pub page_width: Option<f32>,
    /// PDF page height from meta tags
    pub page_height: Option<f32>,
    /// PDF metadata from meta tags
    pub metadata: BTreeMap<String, String>,
    /// Raw component nodes
    pub components: Vec<DynamicXmlComponent>,
}

/// Extract configuration from HTML document
fn extract_config(document: &NodeRef) -> HtmlExtractedConfig {
    let mut config = HtmlExtractedConfig {
        title: None,
        page_width: None,
        page_height: None,
        metadata: BTreeMap::new(),
        components: Vec::new(),
    };

    // Extract title
    if let Some(title_elem) = document.select_first("title").ok() {
        let elem = title_elem.text_contents();
        let s = elem.trim();
        if !s.is_empty() {
            config.title = Some(s.to_string())
        }
    }

    // Extract components from head
    if let Some(head) = document.select_first("head").ok() {
        for component_node in head.as_node().select("component").unwrap() {
            // Clone the component node for later use
            if let Ok(o) = new_dynxml_from_kuchiki(component_node.as_node()) {
                config.components.push(o);
            }
        }
    }

    // Extract metadata from meta tags
    for meta in document.select("meta").unwrap() {
        let node = meta.as_node();
        let attrs = node.as_element().unwrap().attributes.borrow();

        if let (Some(name), Some(content)) = (attrs.get("name"), attrs.get("content")) {
            // Handle PDF options via meta tags
            if name.starts_with("pdf.options.") {
                let option_name = &name["pdf.options.".len()..];
                match option_name {
                    "pageWidth" => {
                        if let Ok(width) = f32::from_str(content) {
                            config.page_width = Some(width);
                        }
                    }
                    "pageHeight" => {
                        if let Ok(height) = f32::from_str(content) {
                            config.page_height = Some(height);
                        }
                    }
                    _ => {} // Ignore other options for now
                }
            } else if name.starts_with("pdf.metadata.") {
                let metadata_key = &name["pdf.metadata.".len()..];
                config
                    .metadata
                    .insert(metadata_key.to_string(), content.to_string());
            }
        }
    }

    config
}

/// Convert the kuchiki document to XML string
fn serialize_to_xml(document: &NodeRef) -> String {
    // For this implementation, we'll create a simplified XML output
    // that includes only elements we can render
    let mut xml = String::new();

    // Start with HTML tag
    xml.push_str("<html>");

    // Process head
    if let Some(head) = document.select_first("head").ok() {
        xml.push_str("<head>");

        // Only include elements we care about (style, component)
        for child in head.as_node().children() {
            match child
                .as_element()
                .map(|s| s.name.local.to_string())
                .as_deref()
            {
                Some("style") => {
                    xml.push_str("<style>");
                    xml.push_str(&child.text_contents());
                    xml.push_str("</style>");
                }
                Some("component") => {
                    // Serialize component with all attributes
                    xml.push_str(&serialize_element(&child));
                }
                _ => {} // Skip other head elements
            }
        }

        xml.push_str("</head>");
    }

    // Process body
    if let Some(body) = document.select_first("body").ok() {
        xml.push_str("<body>");

        // Process all body children recursively
        for child in body.as_node().children() {
            process_node(&child, &mut xml);
        }

        xml.push_str("</body>");
    }

    xml.push_str("</html>");
    xml
}

/// Process a node and add it to the XML output if it's renderable
fn process_node(node: &NodeRef, xml: &mut String) {
    if let Some(name) = node.as_element().map(|s| s.name.local.to_string()) {
        // Check if this is an element we can render
        if is_renderable_element(&name) {
            // Start tag with attributes
            xml.push_str(&serialize_element(node));

            // Process children
            for child in node.children() {
                process_node(&child, xml);
            }

            // End tag
            xml.push_str(&format!("</{}>", name));
        } else if is_custom_component(&name) {
            // Handle custom component (capitalized tag names)
            xml.push_str(&serialize_element(node));

            // Process children if needed
            for child in node.children() {
                process_node(&child, xml);
            }

            // End tag
            xml.push_str(&format!("</{}>", name));
        }
    } else if let Some(t) = node.as_text() {
        // Add text content
        if let Ok(tr) = t.try_borrow() {
            xml.push_str(&escape_xml_text(&*tr));
        }
    }
}

/// Check if an element is one we can render
fn is_renderable_element(name: &str) -> bool {
    // List of HTML elements we support rendering
    let supported_elements = [
        "div", "p", "h1", "h2", "h3", "h4", "h5", "h6", "span", "img", "a", "ul", "ol", "li",
        "table", "tr", "td", "th", "hr", "br", "strong", "em", "b", "i",
    ];

    supported_elements.contains(&name.to_lowercase().as_str())
}

/// Check if the element name appears to be a custom component
fn is_custom_component(name: &str) -> bool {
    // Custom components typically have capitalized first letter in React-style
    !name.is_empty() && name.chars().next().unwrap().is_uppercase()
}

/// Serialize an element opening tag with attributes
fn serialize_element(node: &NodeRef) -> String {
    if let Some(name) = node.as_element().map(|s| s.name.local.to_string()) {
        let mut result = format!("<{}", name);

        // Add attributes
        if let Some(element) = node.as_element() {
            let attrs = element.attributes.borrow();
            for (key, value) in attrs.map.iter() {
                result.push_str(&format!(
                    " {}=\"{}\"",
                    key.local.to_string(),
                    escape_xml_attr(&value.value)
                ));
            }
        }

        // Close the opening tag
        result.push('>');
        result
    } else {
        String::new()
    }
}

/// Escape text for XML
fn escape_xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape attribute values for XML
fn escape_xml_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Apply extracted configuration to the document and rendering options
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
            // ("creationDate", v) => if let Ok(p) = v.parse() { doc.metadata.info.creation_date =
            // p; }, ("metadataDate", v) => if let Ok(p) = v.parse() {
            // doc.metadata.info.metadata_date = p; }, ("modificationDate", v) => if let
            // Ok(p) = v.parse() { doc.metadata.info.modification_date = p; },
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

// Dynamic XML component registration - handle component conversion from kuchiki to XML

/// Create a DynamicXmlComponent from a kuchiki NodeRef
pub fn new_dynxml_from_kuchiki(node: &NodeRef) -> Result<DynamicXmlComponent, ComponentParseError> {
    // Convert kuchiki node to XmlNode format
    let xml_node = kuchiki_to_xml_node(node)?;

    // Use the existing implementation to create the component
    DynamicXmlComponent::new(&xml_node)
}

/// Convert a kuchiki NodeRef to our XmlNode format
fn kuchiki_to_xml_node(node: &NodeRef) -> Result<XmlNode, ComponentParseError> {
    let element = node
        .as_element()
        .ok_or(ComponentParseError::NotAComponent)?;

    let attrs = element.attributes.borrow();

    // Get node type/name
    let node_type = element.name.local.to_string();

    // Create a new XmlNode
    let mut xml_node = XmlNode::new(&*node_type);

    // Copy attributes
    let mut attr = Vec::new();
    for (key, value) in attrs.map.iter() {
        attr.push(AzStringPair {
            key: key.local.trim().to_string().into(),
            value: value.value.trim().to_string().into(),
        });
    }
    xml_node.attributes = attr.into();

    // Process children recursively
    for child in node.children() {
        if child.as_element().is_some() {
            let child_xml = kuchiki_to_xml_node(&child)?;
            xml_node.children.push(child_xml);
        } else if let Some(t) = child.as_text() {
            // If this is a text node, set the text content
            if let Ok(t) = t.try_borrow() {
                xml_node.text = Some(t.clone().into()).into();
            }
        }
    }

    Ok(xml_node)
}

/// Transform component nodes into
pub fn parse_component_nodes(
    nodes: &[NodeRef],
) -> Result<Vec<DynamicXmlComponent>, ComponentParseError> {
    let mut components = Vec::new();

    for node in nodes {
        if let Some(name) = node.as_element().map(|s| s.name.local.to_string()) {
            if name.to_lowercase() == "component" {
                let component = new_dynxml_from_kuchiki(node)?;
                components.push(component);
            }
        }
    }

    Ok(components)
}

/// Convert DynamicXmlComponents to XmlComponents and register them
pub fn register_components(
    component_defs: Vec<DynamicXmlComponent>,
    component_map: &mut XmlComponentMap,
) {
    use azul_core::xml::normalize_casing;

    for component in component_defs {
        let component_name = normalize_casing(&component.name);
        component_map.register_component(XmlComponent {
            id: component_name,
            renderer: Box::new(component),
            inherit_vars: false,
        });
    }
}

/// CSS rule with selectors and declarations
struct CssRule {
    selector: String,
    declarations: Vec<(String, String)>, // Property name, value
}

/// Parse CSS text into a list of rules
fn parse_css(css_text: &str) -> Vec<CssRule> {
    let mut rules = Vec::new();

    // Simple CSS parser (this is a very basic implementation)
    // In a real implementation, use a proper CSS parser library
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

/// Check if an element matches a simple CSS selector
fn element_matches_selector(element: &NodeRef, selector: &str) -> bool {
    // Very basic selector matching - only supports element, class, and ID selectors
    if let Some(element_data) = element.as_element() {
        let name = element
            .as_element()
            .map(|s| s.name.local.to_string())
            .unwrap_or_default();

        // Simple element selector (e.g., "div")
        if selector == name {
            return true;
        }

        // Class selector (e.g., ".my-class")
        if selector.starts_with('.') {
            let class_name = &selector[1..];
            if let Some(class_attr) = element_data.attributes.borrow().get("class") {
                let classes: Vec<&str> = class_attr.split_whitespace().collect();
                return classes.contains(&class_name);
            }
        }

        // ID selector (e.g., "#my-id")
        if selector.starts_with('#') {
            let id_name = &selector[1..];
            if let Some(id_attr) = element_data.attributes.borrow().get("id") {
                return id_attr == id_name;
            }
        }
    }

    false
}

/// Apply CSS rules to matching elements
fn apply_css_rules(document: &NodeRef, rules: &[CssRule]) {
    for rule in rules {
        // Find all elements matching the selector
        for element in document.inclusive_descendants() {
            if element_matches_selector(&element, &rule.selector) {
                if let Some(element_data) = element.as_element() {
                    let mut attributes = element_data.attributes.borrow_mut();

                    // Get or create style attribute
                    let mut style = attributes
                        .get("style")
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    // Add rule declarations to inline style
                    for (property, value) in &rule.declarations {
                        // Only add if not already present in inline style
                        if !style.contains(&format!("{}:", property)) {
                            if !style.is_empty() && !style.ends_with(';') {
                                style.push(';');
                            }
                            style.push_str(&format!("{}:{};", property, value));
                        }
                    }

                    // Set the updated style attribute
                    attributes.insert("style", style);
                }
            }
        }
    }
}

/// Process and inline all CSS in the document
pub fn inline_all_css(document: &NodeRef) {
    // Collect styles from all <style> elements
    let mut css_rules = Vec::new();

    for style_elem in document.select("style").unwrap() {
        let css_text = style_elem.text_contents();
        let rules = parse_css(&css_text);
        css_rules.extend(rules);
    }

    // Apply the collected rules to the document
    apply_css_rules(document, &css_rules);

    // Remove <style> elements after inlining
    for style_elem in document.select("style").unwrap() {
        if let Some(parent) = style_elem.as_node().parent() {
            parent
                .children()
                .filter(|n| n == style_elem.as_node())
                .for_each(|n| n.detach());
        }
    }
}

/// Clean up HTML by removing elements that can't be rendered
pub fn clean_html_for_rendering(document: &NodeRef) {
    let non_renderable_elements = [
        "script", "noscript", "iframe", "canvas", "audio", "video", "source", "track", "embed",
        "object", "param", "picture",
    ];

    for selector in non_renderable_elements.iter() {
        for element in document.select(selector).unwrap() {
            element.as_node().detach();
        }
    }
}

/// Main function to process HTML for rendering
pub fn process_html_for_rendering(html: &str) -> (String, HtmlExtractedConfig) {
    // First, inline all CSS
    let document = kuchiki::parse_html().one(html);

    let config = extract_config(&document);

    clean_html_for_rendering(&document);

    inline_all_css(&document);

    let mut bytes = Vec::new();
    document.serialize(&mut bytes).unwrap();
    let html = String::from_utf8(bytes).unwrap_or_else(|_| html.to_string());

    // Extract configuration
    let document = kuchiki::parse_html().one(html);

    (serialize_to_xml(&document), config)
}
