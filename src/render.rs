use std::collections::BTreeMap;

use base64::Engine;
use serde_derive::{Deserialize, Serialize};

use crate::{
    BlackGenerationExtraFunction, BlackGenerationFunction, BlendMode, Color, CurTransMat,
    ExtendedGraphicsState, FontId, HalftoneType, LineCapStyle, LineDashPattern, LineJoinStyle,
    OutputImageFormat, OverprintMode, PdfResources, Point, Pt, RenderingIntent, SoftMask, TextItem,
    TextMatrix, TextRenderingMode, TransferExtraFunction, TransferFunction,
    UnderColorRemovalExtraFunction, UnderColorRemovalFunction, XObject, XObjectId, ops::PdfPage,
    serialize::prepare_fonts,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfToSvgOptions {
    /// When rendering ImageXObjects, the images are embedded in the SVG.
    /// You can specify here, which image formats you'd like to output, i.e.
    /// `[Jpeg, Png, Avif]` will first try to encode the image to
    /// `image/jpeg,base64=...`, if the encoding fails, it will try
    /// `Png`, and last `Avif`.
    ///
    /// If you want to render the SVG later using `svg2png`, not all image
    /// formats might be supported, but generally in a
    /// browser context you can prefer `WebP` and `Avif` to save space.
    #[serde(default = "default_image_formats_web")]
    pub image_formats: Vec<OutputImageFormat>,
}

impl Default for PdfToSvgOptions {
    fn default() -> Self {
        Self {
            image_formats: vec![
                OutputImageFormat::Png,
                OutputImageFormat::Jpeg,
                OutputImageFormat::Bmp,
            ],
        }
    }
}

fn default_image_formats_web() -> Vec<OutputImageFormat> {
    vec![
        OutputImageFormat::Avif,
        OutputImageFormat::Webp,
        OutputImageFormat::Jpeg,
        OutputImageFormat::Png,
        OutputImageFormat::Bmp,
        OutputImageFormat::Tiff,
        OutputImageFormat::Gif,
        OutputImageFormat::Tga,
    ]
}

impl PdfToSvgOptions {
    pub fn web() -> Self {
        Self {
            image_formats: default_image_formats_web(),
        }
    }
}

/// During rendering this holds all of the "graphics state" of a page, so we
/// can push / save / pop GraphicsStates and restore the state with q / Q operations.
#[derive(Debug, Clone, Default)]
struct GraphicsState {
    text_cursor: Point,
    // TODO: handle BuiltinFont in ExtendedGraphicsState?
    current_font: Option<FontId>,
    transform_matrix: Option<CurTransMat>,
    text_matrix: Option<TextMatrix>,
    fill_color: Option<Color>,
    stroke_color: Option<Color>,
    stroke_width: Option<Pt>,
    dash_array: Option<LineDashPattern>,
    line_join: Option<LineJoinStyle>,
    line_cap: Option<LineCapStyle>,
    character_spacing: Option<f32>,
    line_offset: Option<f32>,
    miter_limit: Option<Pt>,
    horizontal_scaling: Option<f32>,
    text_leading: Option<Pt>,
    rendering_intent: Option<RenderingIntent>,
    text_rendering_mode: Option<TextRenderingMode>,
    marked_content_stack: Vec<String>,
    overprint_mode: Option<OverprintMode>,
    overprint_stroke: bool,
    overprint_fill: bool,
    in_compatibility_section: bool,
    word_spacing: Option<f32>,
    font_sizes: BTreeMap<FontId, Pt>,

    // Extra options
    black_generation: Option<BlackGenerationFunction>,
    black_generation_extra: Option<BlackGenerationExtraFunction>,
    under_color_removal: Option<UnderColorRemovalFunction>,
    under_color_removal_extra: Option<UnderColorRemovalExtraFunction>,
    transfer_function: Option<TransferFunction>,
    transfer_extra_function: Option<TransferExtraFunction>,
    halftone_dictionary: Option<HalftoneType>,
    flatness_tolerance: Option<f32>,
    smoothness_tolerance: Option<f32>,
    stroke_adjustment: Option<bool>,
    blend_mode: Option<BlendMode>,
    soft_mask: Option<SoftMask>,
    current_stroke_alpha: Option<f32>,
    current_fill_alpha: Option<f32>,
    alpha_is_shape: Option<bool>,
    text_knockout: Option<bool>,
}

struct GraphicsStateVec {
    _internal: Vec<GraphicsState>,
}

struct GsInfoCurrent {
    text_cursor: Point,
    current_font: String,
    transform_matrix: CurTransMat,
    text_matrix: TextMatrix,
    fill_color: Color,
    stroke_color: Color,
    stroke_width: Pt,
    dash_array: Option<LineDashPattern>,
    line_join: Option<LineJoinStyle>,
    line_cap: Option<LineCapStyle>,
    character_spacing: Option<f32>,
    line_offset: Option<f32>,
    miter_limit: Option<Pt>,
    horizontal_scaling: Option<f32>,
    text_leading: Option<Pt>,
    rendering_intent: Option<RenderingIntent>,
    text_rendering_mode: Option<TextRenderingMode>,
    marked_content_stack: Vec<String>,
    overprint_mode: Option<OverprintMode>,
    overprint_stroke: bool,
    overprint_fill: bool,
    in_compatibility_section: bool,
    word_spacing: Option<f32>,
    font_sizes: BTreeMap<FontId, Pt>,
}

impl GraphicsStateVec {
    pub fn new() -> Self {
        Self {
            _internal: vec![GraphicsState::default()],
        }
    }
    pub fn save_gs(&mut self) -> Option<()> {
        let last_gs = self._internal.last().cloned()?;
        self._internal.push(last_gs);
        Some(())
    }
    pub fn restore_gs(&mut self) -> Option<()> {
        self._internal.pop();
        if self._internal.is_empty() {
            self._internal.push(GraphicsState::default());
        }
        Some(())
    }
    pub fn load_gs(&mut self, g: &ExtendedGraphicsState) -> Option<()> {
        let last = self._internal.last_mut()?;

        last.overprint_fill = g.overprint_fill;
        last.overprint_stroke = g.overprint_stroke;
        last.current_font = g.font.clone();
        last.dash_array = g.line_dash_pattern.clone();
        last.stroke_width = Some(Pt(g.line_width));
        last.line_cap = Some(g.line_cap);
        last.line_join = Some(g.line_join);
        last.rendering_intent = Some(g.rendering_intent);
        last.overprint_mode = Some(g.overprint_mode);
        last.miter_limit = Some(Pt(g.miter_limit));

        last.black_generation = g.black_generation.clone();
        last.black_generation_extra = g.black_generation_extra.clone();
        last.under_color_removal = g.under_color_removal.clone();
        last.under_color_removal_extra = g.under_color_removal_extra.clone();
        last.transfer_function = g.transfer_function.clone();
        last.transfer_extra_function = g.transfer_extra_function.clone();
        last.halftone_dictionary = g.halftone_dictionary.clone();
        last.soft_mask = g.soft_mask.clone();
        last.flatness_tolerance = Some(g.flatness_tolerance.clone());
        last.smoothness_tolerance = Some(g.smoothness_tolerance.clone());
        last.stroke_adjustment = Some(g.stroke_adjustment.clone());
        last.blend_mode = Some(g.blend_mode.clone());
        last.current_stroke_alpha = Some(g.current_stroke_alpha.clone());
        last.current_fill_alpha = Some(g.current_fill_alpha.clone());
        last.alpha_is_shape = Some(g.alpha_is_shape.clone());
        last.text_knockout = Some(g.text_knockout.clone());

        Some(())
    }
    pub fn set_rendering_intent(&mut self, c: RenderingIntent) -> Option<()> {
        self._internal.last_mut()?.rendering_intent = Some(c);
        Some(())
    }
    pub fn set_horizontal_scaling(&mut self, percent: f32) -> Option<()> {
        self._internal.last_mut()?.horizontal_scaling = Some(percent);
        Some(())
    }
    pub fn set_line_height(&mut self, lh: Pt) -> Option<()> {
        self._internal.last_mut()?.text_leading = Some(lh);
        Some(())
    }
    pub fn set_text_cursor(&mut self, tc: Point) -> Option<()> {
        self._internal.last_mut()?.text_cursor = tc;
        Some(())
    }
    pub fn set_character_spacing(&mut self, cs: f32) -> Option<()> {
        self._internal.last_mut()?.character_spacing = Some(cs);
        Some(())
    }
    pub fn set_cur_trans_mat(&mut self, cm: CurTransMat) -> Option<()> {
        self._internal.last_mut()?.transform_matrix = Some(cm);
        Some(())
    }
    pub fn set_text_mat(&mut self, tm: TextMatrix) -> Option<()> {
        self._internal.last_mut()?.text_matrix = Some(tm);
        Some(())
    }
    pub fn set_dash_pattern(&mut self, da: LineDashPattern) -> Option<()> {
        self._internal.last_mut()?.dash_array = Some(da);
        Some(())
    }
    pub fn set_fill_color(&mut self, c: Color) -> Option<()> {
        self._internal.last_mut()?.fill_color = Some(c);
        Some(())
    }
    pub fn set_outline_color(&mut self, c: Color) -> Option<()> {
        self._internal.last_mut()?.stroke_color = Some(c);
        Some(())
    }
    pub fn set_line_join(&mut self, c: LineJoinStyle) -> Option<()> {
        self._internal.last_mut()?.line_join = Some(c);
        Some(())
    }
    pub fn set_line_cap(&mut self, c: LineCapStyle) -> Option<()> {
        self._internal.last_mut()?.line_cap = Some(c);
        Some(())
    }
    pub fn set_line_offset(&mut self, c: f32) -> Option<()> {
        self._internal.last_mut()?.line_offset = Some(c);
        Some(())
    }
    pub fn set_stroke_width(&mut self, c: Pt) -> Option<()> {
        self._internal.last_mut()?.stroke_width = Some(c);
        Some(())
    }
    pub fn set_word_spacing(&mut self, c: f32) -> Option<()> {
        self._internal.last_mut()?.word_spacing = Some(c);
        Some(())
    }
    pub fn set_font_size(&mut self, font: &FontId, c: Pt) -> Option<()> {
        self._internal
            .last_mut()?
            .font_sizes
            .insert(font.clone(), c);
        Some(())
    }
    pub fn set_miter_limit(&mut self, c: Pt) -> Option<()> {
        self._internal.last_mut()?.miter_limit = Some(c);
        Some(())
    }
    pub fn set_text_rendering_mode(&mut self, c: TextRenderingMode) -> Option<()> {
        self._internal.last_mut()?.text_rendering_mode = Some(c);
        Some(())
    }
    pub fn begin_marked_content(&mut self, tag: String) -> Option<()> {
        self._internal.last_mut()?.marked_content_stack.push(tag);
        Some(())
    }
    pub fn end_marked_content(&mut self) -> Option<()> {
        self._internal.last_mut()?.marked_content_stack.pop();
        Some(())
    }
    pub fn begin_compatibility_section(&mut self) -> Option<()> {
        self._internal.last_mut()?.in_compatibility_section = true;
        Some(())
    }
    pub fn end_compatibility_section(&mut self) -> Option<()> {
        self._internal.last_mut()?.in_compatibility_section = false;
        Some(())
    }
}

pub fn render_to_svg(page: &PdfPage, resources: &PdfResources, opts: &PdfToSvgOptions) -> String {
    let map = encoded_image_data_map(page, resources, opts);
    render_to_svg_internal(page, resources, map)
}

pub async fn render_to_svg_async(
    page: &PdfPage,
    resources: &PdfResources,
    opts: &PdfToSvgOptions,
) -> String {
    let map = encoded_image_data_map_async(page, resources, opts).await;
    render_to_svg_internal(page, resources, map)
}

async fn encoded_image_data_map_async(
    page: &PdfPage,
    resources: &PdfResources,
    opts: &PdfToSvgOptions,
) -> BTreeMap<XObjectId, (Vec<u8>, OutputImageFormat)> {
    let resources_needed = page.get_xobject_ids();
    let mut map = BTreeMap::new();
    for f in resources_needed {
        let encoded = match resources.xobjects.map.get(&f) {
            Some(XObject::Image(i)) => i.encode_to_bytes_async(&opts.image_formats).await,
            _ => continue,
        };
        match encoded {
            Ok(o) => {
                map.insert(f.clone(), o);
            }
            Err(_) => continue,
        }
    }
    map
}

fn encoded_image_data_map(
    page: &PdfPage,
    resources: &PdfResources,
    opts: &PdfToSvgOptions,
) -> BTreeMap<XObjectId, (Vec<u8>, OutputImageFormat)> {
    let resources_needed = page.get_xobject_ids();
    let mut map = BTreeMap::new();
    for f in resources_needed {
        let encoded = match resources.xobjects.map.get(&f) {
            Some(XObject::Image(i)) => i.encode_to_bytes(&opts.image_formats),
            _ => continue,
        };
        match encoded {
            Ok(o) => {
                map.insert(f.clone(), o);
            }
            Err(_) => continue,
        }
    }
    map
}

fn render_to_svg_internal(
    page: &PdfPage,
    resources: &PdfResources,
    map: BTreeMap<XObjectId, (Vec<u8>, OutputImageFormat)>,
) -> String {
    use crate::ops::Op;

    // Extract page dimensions from the media box.
    let width = page.media_box.width.0;
    let height = page.media_box.height.0;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}px" height="{h}px" preserveAspectRatio="xMidYMid meet" viewBox="0 0 {w} {h}">"#,
        w = width,
        h = height
    ));
    svg.push('\n');

    let fonts = prepare_fonts(resources, &[page.clone()]);
    if !fonts.is_empty() {
        // Embed fonts via a <style> block.
        svg.push_str("<style>\n");
        // Iterate over PDF fonts and embed each via an @font-face rule.
        for (font_id, font) in fonts.iter() {
            svg.push_str(&format!(
                r#"@font-face {{ font-family: "{}"; src: url("data:font/otf;charset=utf-8;base64,{}"); }}"#,
                font_id.0, base64::prelude::BASE64_STANDARD.encode(&font.subset_font.bytes),
            ));
        }
        svg.push_str("</style>\n");
    }

    let mut gst = GraphicsStateVec::new();

    for op in &page.ops {
        match op {
            Op::SetRenderingIntent { intent } => {
                gst.set_rendering_intent(*intent);
            }
            Op::SetHorizontalScaling { percent } => {
                gst.set_horizontal_scaling(*percent);
            }
            Op::SetLineOffset { multiplier } => {
                gst.set_line_offset(*multiplier);
            }
            Op::SetLineHeight { lh } => {
                gst.set_line_height(*lh);
            }
            Op::SetTextCursor { pos } => {
                gst.set_text_cursor(*pos);
            }
            Op::SetTransformationMatrix { matrix } => {
                gst.set_cur_trans_mat(matrix.clone());
            }
            Op::SetTextMatrix { matrix } => {
                gst.set_text_mat(matrix.clone());
            }
            Op::SetFillColor { col } => {
                gst.set_fill_color(col.clone());
            }
            Op::SetOutlineColor { col } => {
                gst.set_outline_color(col.clone());
            }
            Op::SetOutlineThickness { pt } => {
                gst.set_stroke_width(*pt);
            }
            Op::SetLineDashPattern { dash } => {
                gst.set_dash_pattern(dash.clone());
            }
            Op::SetLineJoinStyle { join } => {
                gst.set_line_join(*join);
            }
            Op::SetLineCapStyle { cap } => {
                gst.set_line_cap(*cap);
            }
            Op::SetMiterLimit { limit } => {
                gst.set_miter_limit(*limit);
            }
            Op::SetTextRenderingMode { mode } => {
                gst.set_text_rendering_mode(*mode);
            }
            Op::SetCharacterSpacing { multiplier } => {
                gst.set_character_spacing(*multiplier);
            }
            Op::BeginMarkedContent { tag } => {
                gst.begin_marked_content(tag.clone());
            }
            Op::BeginMarkedContentWithProperties { tag, properties: _ } => {
                gst.begin_marked_content(tag.clone());
            }
            Op::DefineMarkedContentPoint { tag, properties: _ } => {
                gst.begin_marked_content(tag.clone());
            }
            Op::EndMarkedContent => {
                gst.end_marked_content();
            }
            Op::BeginCompatibilitySection => {
                gst.begin_compatibility_section();
            }
            Op::EndCompatibilitySection => {
                gst.end_compatibility_section();
            }
            Op::Marker { id } => {
                svg.push_str(&format!(
                    "<div class='marker' id='{id}' style='display:none;' />"
                ));
            }
            Op::BeginLayer { layer_id } => {
                svg.push_str(&format!("<div class='layer' id='{}'>", layer_id.0));
            }
            Op::EndLayer { .. } => {
                svg.push_str(&format!("</div>"));
            }
            Op::SaveGraphicsState => {
                gst.save_gs();
            }
            Op::RestoreGraphicsState => {
                gst.restore_gs();
            }
            Op::LoadGraphicsState { gs } => {
                resources
                    .extgstates
                    .map
                    .get(gs)
                    .and_then(|s| gst.load_gs(s));
            }
            Op::SetWordSpacing { percent } => {
                gst.set_word_spacing(*percent);
            }
            Op::SetFontSize { size, font } => {
                gst.set_font_size(font, *size);
            }

            Op::BeginInlineImage => { /* (For now, possibly note that an inline image is beginning.) */
            }
            Op::BeginInlineImageData => { /* … */ }
            Op::EndInlineImage => { /* … */ }

            // Text moving operators (depend on font size / font)
            Op::MoveTextCursorAndSetLeading { tx, ty } => todo!(),
            Op::AddLineBreak => {}
            Op::MoveToNextLineShowText { text } => {
                // (Render text on a new line; exact implementation depends on your SVG output.)
            }
            Op::SetSpacingMoveAndShowText {
                word_spacing,
                char_spacing,
                text,
            } => {
                // (Render text with the specified spacing; implement as needed.)
            }
            // Actual rendering
            Op::LinkAnnotation { link } => {
                /*
                LinkAnnotation {
                    pub rect: Rect,
                    pub actions: Actions,

                    #[serde(default)]
                    pub border: BorderArray,
                    #[serde(default)]
                    pub color: ColorArray,
                    #[serde(default)]
                    pub highlighting: HighlightingMode,
                }*/
            }
            Op::StartTextSection => {
                svg.push_str("<text>");
            }
            Op::EndTextSection => {
                svg.push_str("</text>");
            }
            Op::WriteText { items, size, font } => {}
            Op::WriteTextBuiltinFont { items, size, font } => {}
            Op::WriteCodepoints { font, size, cp } => {}
            Op::WriteCodepointsWithKerning { font, size, cpk } => {}
            Op::DrawLine { line } => {}
            Op::DrawPolygon { polygon } => {}
            Op::UseXobject { id, transform } => {
                let rmap = resources
                    .xobjects
                    .map
                    .get(id)
                    .and_then(|s| Some((s, map.get(id)?)));
                if let Some((raw_image, (bytes, fmt))) = rmap {
                    let (w, h) = raw_image
                        .get_width_height()
                        .map(|(w, h)| (w.0, h.0))
                        .unwrap_or((0, 0));
                    let base64_str = base64::prelude::BASE64_STANDARD.encode(&bytes);
                    let data_url = format!("data:{};base64,{}", fmt.mime_type(), base64_str);
                    svg.push_str(&format!(
                        "<image width=\"{w}\" height=\"{h}\" xlink:href=\"{data_url}\" />"
                    ));
                }
            }
            Op::Unknown { key, value } => {}
        }
    }

    svg.push_str("</svg>");
    svg
}

// In render.rs
fn render_text_to_svg(items: &[TextItem], size: Pt, font_family: &str) -> String {
    let mut result = String::new();
    let mut x_offset = 0.0;

    for item in items {
        match item {
            TextItem::Text(text) => {
                // Escape text for XML
                let escaped = escape_xml_text(text);

                // Add a tspan if we have an offset
                if x_offset != 0.0 {
                    result.push_str(&format!("<tspan dx=\"{}\">{}</tspan>", x_offset, escaped));
                    x_offset = 0.0;
                } else {
                    result.push_str(&escaped);
                }
            }
            TextItem::Offset(offset) => {
                // Convert offset from thousandths of an em to actual points
                // Negative numbers in TJ mean add space, positive mean remove space
                let em_size = size.0; // Size in points
                x_offset += -*offset as f32 * em_size / 1000.0;
            }
        }
    }

    result
}

fn escape_xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
