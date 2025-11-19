use std::collections::BTreeMap;

use base64::Engine;
use serde_derive::{Deserialize, Serialize};

use crate::{
    ops::PdfPage, Actions, BlackGenerationExtraFunction,
    BlackGenerationFunction, BlendMode, BuiltinFont, BuiltinOrExternalFontId, ChangedField, Color,
    CurTransMat, Destination, ExtendedGraphicsState, FontId, HalftoneType, Line, LineCapStyle,
    LineDashPattern, LineJoinStyle, OutputImageFormat, OverprintMode, PaintMode, PdfResources,
    PdfWarnMsg, Point, Polygon, Pt, RenderingIntent, SoftMask, TextItem, TextMatrix,
    TextRenderingMode, TransferExtraFunction, TransferFunction, UnderColorRemovalExtraFunction,
    UnderColorRemovalFunction, WindingOrder, XObject, XObjectId, XObjectTransform,
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
    current_font: Option<BuiltinOrExternalFontId>,
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
    word_spacing: Option<Pt>,
    font_sizes: BTreeMap<BuiltinOrExternalFontId, Pt>,

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

        // Only apply fields that are marked as changed
        if g.changed_fields.contains(&ChangedField::OverprintFill) {
            last.overprint_fill = g.overprint_fill;
        }
        if g.changed_fields.contains(&ChangedField::OverprintStroke) {
            last.overprint_stroke = g.overprint_stroke;
        }
        if g.changed_fields.contains(&ChangedField::Font) {
            last.current_font = g.font.clone();
        }
        if g.changed_fields.contains(&ChangedField::LineDashPattern) {
            last.dash_array = g.line_dash_pattern.clone();
        }
        if g.changed_fields.contains(&ChangedField::LineWidth) {
            last.stroke_width = Some(Pt(g.line_width));
        }
        if g.changed_fields.contains(&ChangedField::LineCap) {
            last.line_cap = Some(g.line_cap);
        }
        if g.changed_fields.contains(&ChangedField::LineJoin) {
            last.line_join = Some(g.line_join);
        }
        if g.changed_fields.contains(&ChangedField::RenderingIntent) {
            last.rendering_intent = Some(g.rendering_intent);
        }
        if g.changed_fields.contains(&ChangedField::OverprintMode) {
            last.overprint_mode = Some(g.overprint_mode);
        }
        if g.changed_fields.contains(&ChangedField::MiterLimit) {
            last.miter_limit = Some(Pt(g.miter_limit));
        }
        if g.changed_fields.contains(&ChangedField::BlackGeneration) {
            last.black_generation = g.black_generation.clone();
        }
        if g.changed_fields
            .contains(&ChangedField::BlackGenerationExtra)
        {
            last.black_generation_extra = g.black_generation_extra.clone();
        }
        if g.changed_fields.contains(&ChangedField::UnderColorRemoval) {
            last.under_color_removal = g.under_color_removal.clone();
        }
        if g.changed_fields
            .contains(&ChangedField::UnderColorRemovalExtra)
        {
            last.under_color_removal_extra = g.under_color_removal_extra.clone();
        }
        if g.changed_fields.contains(&ChangedField::TransferFunction) {
            last.transfer_function = g.transfer_function.clone();
        }
        if g.changed_fields
            .contains(&ChangedField::TransferFunctionExtra)
        {
            last.transfer_extra_function = g.transfer_extra_function.clone();
        }
        if g.changed_fields.contains(&ChangedField::HalftoneDictionary) {
            last.halftone_dictionary = g.halftone_dictionary.clone();
        }
        if g.changed_fields.contains(&ChangedField::SoftMask) {
            last.soft_mask = g.soft_mask.clone();
        }
        if g.changed_fields.contains(&ChangedField::FlatnessTolerance) {
            last.flatness_tolerance = Some(g.flatness_tolerance);
        }
        if g.changed_fields
            .contains(&ChangedField::SmoothnessTolerance)
        {
            last.smoothness_tolerance = Some(g.smoothness_tolerance);
        }
        if g.changed_fields.contains(&ChangedField::StrokeAdjustment) {
            last.stroke_adjustment = Some(g.stroke_adjustment);
        }
        if g.changed_fields.contains(&ChangedField::BlendMode) {
            last.blend_mode = Some(g.blend_mode.clone());
        }
        if g.changed_fields.contains(&ChangedField::CurrentStrokeAlpha) {
            last.current_stroke_alpha = Some(g.current_stroke_alpha);
        }
        if g.changed_fields.contains(&ChangedField::CurrentFillAlpha) {
            last.current_fill_alpha = Some(g.current_fill_alpha);
        }
        if g.changed_fields.contains(&ChangedField::AlphaIsShape) {
            last.alpha_is_shape = Some(g.alpha_is_shape);
        }
        if g.changed_fields.contains(&ChangedField::TextKnockout) {
            last.text_knockout = Some(g.text_knockout);
        }

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
    pub fn set_word_spacing(&mut self, c: Pt) -> Option<()> {
        self._internal.last_mut()?.word_spacing = Some(c);
        Some(())
    }
    pub fn set_font_size(&mut self, font: &FontId, c: Pt) -> Option<()> {
        self._internal
            .last_mut()?
            .font_sizes
            .insert(BuiltinOrExternalFontId::External(font.clone()), c);
        Some(())
    }
    pub fn set_font_size_builtin(&mut self, font: &BuiltinFont, c: Pt) -> Option<()> {
        self._internal
            .last_mut()?
            .font_sizes
            .insert(BuiltinOrExternalFontId::Builtin(*font), c);
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

    pub fn get_current(&self) -> Option<&GraphicsState> {
        self._internal.last()
    }

    pub fn get_font_size(&self, font: &BuiltinOrExternalFontId) -> Pt {
        self.get_current()
            .and_then(|gs| gs.font_sizes.get(font))
            .copied()
            .unwrap_or(Pt(12.0)) // Default font size
    }

    pub fn get_text_cursor(&self) -> Point {
        self.get_current()
            .map(|gs| gs.text_cursor)
            .unwrap_or_default()
    }

    pub fn get_transform_matrix(&self) -> CurTransMat {
        self.get_current()
            .and_then(|gs| gs.transform_matrix)
            .unwrap_or(CurTransMat::Identity)
    }

    pub fn get_text_matrix(&self) -> TextMatrix {
        self.get_current()
            .and_then(|gs| gs.text_matrix)
            .unwrap_or(TextMatrix::Translate(Pt(0.0), Pt(0.0)))
    }

    pub fn get_fill_color(&self) -> Color {
        self.get_current()
            .and_then(|gs| gs.fill_color.clone())
            .unwrap_or(Color::Rgb(crate::Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }))
    }

    pub fn get_stroke_color(&self) -> Color {
        self.get_current()
            .and_then(|gs| gs.stroke_color.clone())
            .unwrap_or(Color::Rgb(crate::Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }))
    }

    pub fn get_stroke_width(&self) -> Pt {
        self.get_current()
            .and_then(|gs| gs.stroke_width)
            .unwrap_or(Pt(1.0))
    }

    pub fn get_dash_array(&self) -> Option<LineDashPattern> {
        self.get_current()?.dash_array.clone()
    }

    pub fn get_line_join(&self) -> LineJoinStyle {
        self.get_current()
            .and_then(|gs| gs.line_join)
            .unwrap_or(LineJoinStyle::Miter)
    }

    pub fn get_line_cap(&self) -> LineCapStyle {
        self.get_current()
            .and_then(|gs| gs.line_cap)
            .unwrap_or(LineCapStyle::Butt)
    }

    pub fn get_text_rendering_mode(&self) -> TextRenderingMode {
        self.get_current()
            .and_then(|gs| gs.text_rendering_mode)
            .unwrap_or(TextRenderingMode::Fill)
    }

    pub fn get_character_spacing(&self) -> f32 {
        self.get_current()
            .and_then(|gs| gs.character_spacing)
            .unwrap_or(0.0)
    }

    pub fn get_word_spacing(&self) -> f32 {
        self.get_current()
            .and_then(|gs| gs.word_spacing)
            .unwrap_or(Pt(0.0))
            .0
    }

    pub fn get_text_leading(&self) -> Pt {
        self.get_current()
            .and_then(|gs| gs.text_leading)
            .unwrap_or(Pt(0.0))
    }

    pub fn get_horizontal_scaling(&self) -> f32 {
        self.get_current()
            .and_then(|gs| gs.horizontal_scaling)
            .unwrap_or(100.0)
    }

    pub fn get_current_font(&self) -> Option<BuiltinOrExternalFontId> {
        self.get_current()?.current_font.clone()
    }
}

pub fn render_to_svg(
    page: &PdfPage,
    resources: &PdfResources,
    opts: &PdfToSvgOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> String {
    let map = encoded_image_data_map(page, resources, opts);
    render_to_svg_internal(page, resources, map, warnings)
}

pub async fn render_to_svg_async(
    page: &PdfPage,
    resources: &PdfResources,
    opts: &PdfToSvgOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> String {
    let map = encoded_image_data_map_async(page, resources, opts).await;
    render_to_svg_internal(page, resources, map, warnings)
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
    warnings: &mut Vec<PdfWarnMsg>,
) -> String {
    use crate::ops::Op;

    // Extract page dimensions from the media box
    let width = page.media_box.width.0;
    let height = page.media_box.height.0;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}px" height="{h}px" viewBox="0 0 {w} {h}">"#,
        w = width,
        h = height
    ));
    svg.push('\n');

    // Handle fonts
    let fonts = crate::serialize::prepare_fonts_for_serialization(resources, &[page.clone()], warnings);
    if !fonts.is_empty() {
        // Embed fonts via a <style> block
        svg.push_str("<style>\n");
        // Iterate over PDF fonts and embed each via an @font-face rule
        for (font_id, font) in fonts.iter() {
            svg.push_str(&format!(
                r#"@font-face {{ font-family: "{}"; src: url("data:font/otf;charset=utf-8;base64,{}"); }}"#,
                font_id.0, base64::prelude::BASE64_STANDARD.encode(&font.subset_font_bytes),
            ));
        }
        svg.push_str("</style>\n");
    }

    // Initialize graphics state and tracking variables
    let mut gst = GraphicsStateVec::new();
    let mut in_text_section = false;
    let mut current_svg_group = Vec::new(); // Stack to track nested groups

    // Process all PDF operations
    for op in &page.ops {
        match op {
            Op::SetColorSpaceFill { .. } => {
                // TODO
            }
            Op::SetColorSpaceStroke { .. } => {
                // TODO
            }
            // Graphics state modifications
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
            Op::SetWordSpacing { pt } => {
                gst.set_word_spacing(*pt);
            }
            Op::SetFont { font, size } => {
                // Use PdfFontHandle enum to set font
                match font {
                    crate::ops::PdfFontHandle::Builtin(builtin) => {
                        gst.set_font_size_builtin(builtin, *size);
                    }
                    crate::ops::PdfFontHandle::External(font_id) => {
                        gst.set_font_size(font_id, *size);
                    }
                }
            }
            Op::ShowText { items } => {
                // ShowText requires that font was set previously via SetFont
                // The renderer tracks current font in GraphicsState
                if in_text_section {
                    if let Some(current_font) = gst.get_current_font() {
                        let text_svg = render_text_items_to_svg(
                            items,
                            &current_font,
                            &gst,
                            height,
                        );
                        svg.push_str(&text_svg);
                    } else {
                        // No font set - use default and emit warning if configured
                        let default_font = BuiltinOrExternalFontId::Builtin(BuiltinFont::default());
                        let text_svg = render_text_items_to_svg(
                            items,
                            &default_font,
                            &gst,
                            height,
                        );
                        svg.push_str(&text_svg);
                    }
                }
            }

            // Content structure
            Op::BeginMarkedContent { tag } => {
                gst.begin_marked_content(tag.clone());
                svg.push_str(&format!(
                    "<g class=\"marked-content\" data-tag=\"{}\">",
                    tag
                ));
                current_svg_group.push(String::from("marked"));
            }
            Op::BeginMarkedContentWithProperties { tag, properties: _ } => {
                gst.begin_marked_content(tag.clone());
                svg.push_str(&format!(
                    "<g class=\"marked-content\" data-tag=\"{}\">",
                    tag
                ));
                current_svg_group.push(String::from("marked"));
            }
            Op::DefineMarkedContentPoint { tag, properties: _ } => {
                svg.push_str(&format!(
                    "<g class=\"marked-content-point\" data-tag=\"{}\"></g>",
                    tag
                ));
            }
            Op::EndMarkedContent | Op::EndMarkedContentWithProperties => {
                gst.end_marked_content();
                if let Some(group_type) = current_svg_group.last() {
                    if group_type == "marked" {
                        current_svg_group.pop();
                        svg.push_str("</g>");
                    }
                }
            }
            Op::BeginCompatibilitySection => {
                gst.begin_compatibility_section();
                // We'll skip content in compatibility sections
            }
            Op::EndCompatibilitySection => {
                gst.end_compatibility_section();
            }
            Op::Marker { id } => {
                svg.push_str(&format!(
                    "<g class='marker' id='{}' style='display:none;'></g>",
                    id
                ));
            }
            Op::BeginLayer { layer_id } | Op::BeginOptionalContent { layer_id } => {
                if let Some(layer) = resources.layers.map.get(layer_id) {
                    svg.push_str(&format!(
                        "<g class=\"layer\" id=\"{}\" data-name=\"{}\">",
                        layer_id.0, layer.name
                    ));
                } else {
                    svg.push_str(&format!("<g class=\"layer\" id=\"{}\">", layer_id.0));
                }
                current_svg_group.push(String::from("layer"));
            }
            Op::EndLayer | Op::EndOptionalContent => {
                if let Some(group_type) = current_svg_group.last() {
                    if group_type == "layer" {
                        current_svg_group.pop();
                        svg.push_str("</g>");
                    }
                }
            }
            Op::SaveGraphicsState => {
                gst.save_gs();
                svg.push_str("<g>"); // Group to isolate graphics state changes
                current_svg_group.push(String::from("gs"));
            }
            Op::RestoreGraphicsState => {
                gst.restore_gs();
                if let Some(group_type) = current_svg_group.last() {
                    if group_type == "gs" {
                        current_svg_group.pop();
                        svg.push_str("</g>");
                    }
                }
            }
            Op::LoadGraphicsState { gs } => {
                if let Some(graphics_state) = resources.extgstates.map.get(gs) {
                    gst.load_gs(graphics_state);
                }
            }

            // Text operations
            Op::StartTextSection => {
                in_text_section = true;
                // We don't create a text element here; each WriteText call will do that
            }
            Op::EndTextSection => {
                in_text_section = false;
            }
            Op::AddLineBreak => {
                // For line breaks, we adjust the text cursor using the current leading
                let leading = gst.get_text_leading();
                let current_cursor = gst.get_text_cursor();
                gst.set_text_cursor(Point {
                    x: Pt(0.0),                            // Reset X to start of line
                    y: Pt(current_cursor.y.0 - leading.0), // Move down by leading amount
                });
            }
            Op::MoveTextCursorAndSetLeading { tx, ty } => {
                // Update cursor position and set leading
                let current_cursor = gst.get_text_cursor();
                gst.set_text_cursor(Point {
                    x: Pt(current_cursor.x.0 + tx),
                    y: Pt(current_cursor.y.0 + ty),
                });
                gst.set_line_height(Pt(-ty)); // Leading is -ty
            }
            Op::MoveToNextLineShowText { text } => {
                if in_text_section {
                    // First move to next line using leading
                    let leading = gst.get_text_leading();
                    let current_cursor = gst.get_text_cursor();
                    gst.set_text_cursor(Point {
                        x: Pt(0.0),                            // Reset X to start of line
                        y: Pt(current_cursor.y.0 - leading.0), // Move down by leading
                    });

                    // Then show text
                    let items = vec![TextItem::Text(text.clone())];

                    if let Some(font) = gst.get_current_font() {
                        let text_svg = render_text_items_to_svg(&items, &font, &gst, height);
                        svg.push_str(&text_svg);
                    }
                }
            }
            Op::SetSpacingMoveAndShowText {
                word_spacing,
                char_spacing,
                text,
            } => {
                if in_text_section {
                    // Set spacing
                    gst.set_word_spacing(Pt(*word_spacing));
                    gst.set_character_spacing(*char_spacing);

                    // Move to next line
                    let leading = gst.get_text_leading();
                    let current_cursor = gst.get_text_cursor();
                    gst.set_text_cursor(Point {
                        x: Pt(0.0),                            // Reset X to start of line
                        y: Pt(current_cursor.y.0 - leading.0), // Move down by leading
                    });

                    // Show text
                    let items = vec![TextItem::Text(text.clone())];

                    if let Some(font) = gst.get_current_font() {
                        let text_svg = render_text_items_to_svg(&items, &font, &gst, height);
                        svg.push_str(&text_svg);
                    }
                }
            }

            // Drawing operations
            Op::DrawLine { line } => {
                let line_svg = render_line_to_svg(line, &gst, height);
                svg.push_str(&line_svg);
            }
            Op::DrawPolygon { polygon } => {
                let polygon_svg = render_polygon_to_svg(polygon, &gst, height);
                svg.push_str(&polygon_svg);
            }
            Op::DrawRectangle { rectangle } => {
                let polygon_svg = render_polygon_to_svg(&rectangle.to_polygon(), &gst, height);
                svg.push_str(&polygon_svg);
            }
            Op::UseXobject { id, transform } => {
                let xobject_svg = render_image_to_svg(id, transform, resources, &map, height, &gst);
                svg.push_str(&xobject_svg);
            }
            Op::LinkAnnotation { link } => {
                // Render link annotations as SVG links
                match &link.actions {
                    Actions::Goto(destination) => {
                        // Internal link to a page
                        let page_num = match destination {
                            Destination::Xyz { page, .. } => page,
                            // Handle other destination types if needed
                        };

                        // Create SVG link
                        svg.push_str(&format!("<a href=\"#page{}\">", page_num));

                        // Add rectangle for the link area
                        svg.push_str(&format!(
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" \
                             stroke=\"none\" pointer-events=\"all\"/>",
                            link.rect.x.0,
                            height - link.rect.y.0 - link.rect.height.0, // Convert Y coordinate
                            link.rect.width.0,
                            link.rect.height.0
                        ));

                        svg.push_str("</a>");
                    }
                    Actions::Uri(uri) => {
                        // External link
                        svg.push_str(&format!("<a href=\"{}\" target=\"_blank\">", uri));

                        // Add rectangle for the link area
                        svg.push_str(&format!(
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" \
                             stroke=\"none\" pointer-events=\"all\"/>",
                            link.rect.x.0,
                            height - link.rect.y.0 - link.rect.height.0, // Convert Y coordinate
                            link.rect.width.0,
                            link.rect.height.0
                        ));

                        svg.push_str("</a>");
                    }
                }
            }

            // Inline image operations - simplified implementation
            Op::BeginInlineImage | Op::BeginInlineImageData | Op::EndInlineImage => {
                // These operations would need more complex handling
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    "Inline image rendering not fully implemented in SVG output".to_string(),
                ));
            }

            // Unknown operations
            Op::Unknown { key, value: _ } => {
                // Add comment for debugging
                svg.push_str(&format!("<!-- Unknown PDF operator: {} -->", key));
            }
        }
    }

    // Close any remaining open groups
    for _group_type in current_svg_group.iter().rev() {
        svg.push_str("</g>");
    }

    svg.push_str("</svg>");
    svg
}

// Helper function to convert a Color to SVG color string
fn color_to_svg(color: &Color) -> String {
    match color {
        Color::Rgb(rgb) => {
            let r = (rgb.r * 255.0).round() as u8;
            let g = (rgb.g * 255.0).round() as u8;
            let b = (rgb.b * 255.0).round() as u8;
            format!("rgb({}, {}, {})", r, g, b)
        }
        Color::Cmyk(cmyk) => {
            // Convert CMYK to RGB for SVG
            let r = (1.0 - cmyk.c) * (1.0 - cmyk.k);
            let g = (1.0 - cmyk.m) * (1.0 - cmyk.k);
            let b = (1.0 - cmyk.y) * (1.0 - cmyk.k);
            let r = (r * 255.0).round() as u8;
            let g = (g * 255.0).round() as u8;
            let b = (b * 255.0).round() as u8;
            format!("rgb({}, {}, {})", r, g, b)
        }
        Color::Greyscale(gs) => {
            let v = (gs.percent * 255.0).round() as u8;
            format!("rgb({}, {}, {})", v, v, v)
        }
        Color::SpotColor(spot) => {
            // Convert spot color to RGB for SVG
            let r = (1.0 - spot.c) * (1.0 - spot.k);
            let g = (1.0 - spot.m) * (1.0 - spot.k);
            let b = (1.0 - spot.y) * (1.0 - spot.k);
            let r = (r * 255.0).round() as u8;
            let g = (g * 255.0).round() as u8;
            let b = (b * 255.0).round() as u8;
            format!("rgb({}, {}, {})", r, g, b)
        }
    }
}

// Escapes text for use in SVG
fn escape_xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// Helper to transform a point from PDF to SVG coordinates
fn transform_point(
    point: &Point,
    ctm: &CurTransMat,
    text_matrix: &TextMatrix,
    page_height: f32,
) -> (f32, f32) {
    // First apply text matrix if present
    let (mut tx, mut ty) = (point.x.0, point.y.0);

    // Apply CTM first
    let ctm_array = ctm.as_array();
    tx = ctm_array[0] * tx + ctm_array[2] * ty + ctm_array[4];
    ty = ctm_array[1] * tx + ctm_array[3] * ty + ctm_array[5];

    // Apply text matrix
    let tm_array = text_matrix.as_array();
    tx = tm_array[0] * tx + tm_array[2] * ty + tm_array[4];
    ty = tm_array[1] * tx + tm_array[3] * ty + tm_array[5];

    // Convert to SVG coordinates (flip Y)
    (tx, page_height - ty)
}

// Gets combined transform string for SVG elements
fn get_svg_transform(
    ctm: &CurTransMat,
    text_matrix: &Option<TextMatrix>,
    page_height: f32,
) -> String {
    let mut transform = String::new();

    // Apply CTM first
    let ctm_array = ctm.as_array();
    if *ctm != CurTransMat::Identity {
        // SVG coordinate system flips Y compared to PDF
        transform.push_str(&format!(
            "matrix({} {} {} {} {} {})",
            ctm_array[0],
            -ctm_array[1],
            -ctm_array[2],
            ctm_array[3],
            ctm_array[4],
            page_height - ctm_array[5]
        ));
    }

    // Then apply text matrix if present
    if let Some(tm) = text_matrix {
        let tm_array = tm.as_array();
        if !transform.is_empty() {
            transform.push(' ');
        }
        transform.push_str(&format!(
            "matrix({} {} {} {} {} {})",
            tm_array[0],
            -tm_array[1],
            -tm_array[2],
            tm_array[3],
            tm_array[4],
            -tm_array[5] // No page_height adjustment needed here
        ));
    }

    transform
}

// Renders text items (with offsets) to SVG
fn render_text_items_to_svg(
    items: &[TextItem],
    font_id: &BuiltinOrExternalFontId,
    gst: &GraphicsStateVec,
    page_height: f32,
) -> String {
    let mut result = String::new();

    // Get font size
    let font_size = gst.get_font_size(font_id);

    // Get font family name
    let font_family = match font_id {
        BuiltinOrExternalFontId::Builtin(builtin_font) => builtin_font.get_svg_font_family(),
        BuiltinOrExternalFontId::External(font_id) => &font_id.0, /* Use external font ID as
                                                                   * family name */
    };

    // Get current text cursor position
    let cursor = gst.get_text_cursor();

    // Apply transformations: convert from PDF to SVG coordinates
    let (x, y) = transform_point(
        &cursor,
        &gst.get_transform_matrix(),
        &gst.get_text_matrix(),
        page_height,
    );

    // Get text rendering mode styling
    let text_mode = gst.get_text_rendering_mode();
    let (fill, stroke) = match text_mode {
        TextRenderingMode::Fill => (color_to_svg(&gst.get_fill_color()), "none".to_string()),
        TextRenderingMode::Stroke => ("none".to_string(), color_to_svg(&gst.get_stroke_color())),
        TextRenderingMode::FillStroke => (
            color_to_svg(&gst.get_fill_color()),
            color_to_svg(&gst.get_stroke_color()),
        ),
        TextRenderingMode::Invisible => ("none".to_string(), "none".to_string()),
        TextRenderingMode::FillClip => (color_to_svg(&gst.get_fill_color()), "none".to_string()),
        TextRenderingMode::StrokeClip => {
            ("none".to_string(), color_to_svg(&gst.get_stroke_color()))
        }
        TextRenderingMode::FillStrokeClip => (
            color_to_svg(&gst.get_fill_color()),
            color_to_svg(&gst.get_stroke_color()),
        ),
        TextRenderingMode::Clip => ("none".to_string(), "none".to_string()),
    };

    // Get other text style properties
    let stroke_width = gst.get_stroke_width().0;
    let font_weight = match font_id {
        BuiltinOrExternalFontId::Builtin(bf) => bf.get_font_weight(),
        _ => "normal",
    };
    let font_style = match font_id {
        BuiltinOrExternalFontId::Builtin(bf) => bf.get_font_style(),
        _ => "normal",
    };

    // Process text content with any offsets
    let mut processed_text = String::new();
    let mut x_offset = 0.0;

    for item in items {
        match item {
            TextItem::Text(text) => {
                // Escape text for XML
                let escaped = escape_xml_text(text);

                if x_offset != 0.0 {
                    // Use tspan with dx for positioning if we have an offset
                    processed_text
                        .push_str(&format!("<tspan dx=\"{}\">{}</tspan>", x_offset, escaped));
                    x_offset = 0.0;
                } else {
                    processed_text.push_str(&escaped);
                }
            }
            TextItem::GlyphIds(glyphs) => {
                // Render glyph IDs as placeholder
                // TODO: Convert GIDs to unicode using font mapping
                processed_text.push_str(&format!("[{} glyphs]", glyphs.len()));
            }
            TextItem::Offset(offset) => {
                // Convert from thousandths of an em to points
                // In PDF, positive offset means moving LEFT (decreasing X)
                x_offset -= *offset as f32 * font_size.0 / 1000.0;
            }
        }
    }

    // Get character spacing and word spacing
    let char_spacing = gst.get_character_spacing();
    let word_spacing = gst.get_word_spacing();

    // Get scaling factor if set
    let h_scale = gst.get_horizontal_scaling() / 100.0; // Convert percentage to multiplier

    // Get transform combining CTM and text matrix
    let transform = get_svg_transform(
        &gst.get_transform_matrix(),
        &gst.get_current().and_then(|gs| gs.text_matrix.clone()),
        page_height,
    );

    // Create the SVG text element
    result.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\" \
         font-style=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"",
        x, y, font_family, font_size.0, font_weight, font_style, fill, stroke, stroke_width
    ));

    // Add optional attributes
    if !transform.is_empty() {
        result.push_str(&format!(" transform=\"{}\"", transform));
    }

    if char_spacing != 0.0 {
        result.push_str(&format!(" letter-spacing=\"{}\"", char_spacing));
    }

    if word_spacing != 0.0 {
        result.push_str(&format!(" word-spacing=\"{}\"", word_spacing));
    }

    if h_scale != 1.0 {
        // Apply horizontal scaling via transform
        result.push_str(&format!(" transform=\"scale({}, 1)\"", h_scale));
    }

    // Close tag opening and add content
    result.push_str(&format!(">{}</text>", processed_text));

    result
}

// Renders a line to SVG
fn render_line_to_svg(line: &Line, gst: &GraphicsStateVec, page_height: f32) -> String {
    if line.points.is_empty() {
        return String::new();
    }

    // Generate SVG path data
    let mut path_data = String::new();

    // Start with the first point
    let first_point = &line.points[0];
    path_data.push_str(&format!(
        "M{},{}",
        first_point.p.x.0,
        page_height - first_point.p.y.0
    ));

    // Process remaining points, handling bezier control points
    let mut i = 1;
    while i < line.points.len() {
        let point = &line.points[i];

        if point.bezier && i + 2 <= line.points.len() {
            // This is a bezier control point
            let next_point = &line.points[i + 1];
            path_data.push_str(&format!(
                " Q{},{} {},{}",
                point.p.x.0,
                page_height - point.p.y.0,
                next_point.p.x.0,
                page_height - next_point.p.y.0
            ));
            i += 2; // Skip the next point as it's the end of the bezier
        } else {
            // Regular line segment
            path_data.push_str(&format!(" L{},{}", point.p.x.0, page_height - point.p.y.0));
            i += 1;
        }
    }

    if line.is_closed {
        path_data.push_str(" Z"); // Close the path
    }

    // Get styling from graphics state
    let stroke = color_to_svg(&gst.get_stroke_color());
    let stroke_width = gst.get_stroke_width().0;
    let line_join = gst.get_line_join().to_svg_string();
    let line_cap = gst.get_line_cap().get_svg_id();

    // Handle dash pattern if present
    let dash_array = match gst.get_dash_array() {
        Some(dash) => {
            let dash_array = dash.as_array();
            if dash_array.is_empty() {
                String::new()
            } else {
                format!(
                    " stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\"",
                    dash_array
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                    dash.offset
                )
            }
        }
        None => String::new(),
    };

    // Get transformation if present
    let transform = get_svg_transform(
        &gst.get_transform_matrix(),
        &None, // Line doesn't use text matrix
        page_height,
    );

    let transform_attr = if !transform.is_empty() {
        format!(" transform=\"{}\"", transform)
    } else {
        String::new()
    };

    // Create the SVG path element
    format!(
        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"{}\" \
         stroke-linecap=\"{}\"{}{} />",
        path_data, stroke, stroke_width, line_join, line_cap, dash_array, transform_attr
    )
}

// Renders a polygon to SVG
fn render_polygon_to_svg(polygon: &Polygon, gst: &GraphicsStateVec, page_height: f32) -> String {
    if polygon.rings.is_empty() {
        return String::new();
    }

    // Generate SVG path data
    let mut path_data = String::new();

    for ring in &polygon.rings {
        if ring.points.is_empty() {
            continue;
        }

        // Start with the first point of this ring
        let first_point = &ring.points[0];
        path_data.push_str(&format!(
            "M{},{}",
            first_point.p.x.0,
            page_height - first_point.p.y.0
        ));

        // Process remaining points, handling bezier curves
        let mut i = 1;
        while i < ring.points.len() {
            let point = &ring.points[i];

            if point.bezier && i + 2 <= ring.points.len() {
                // This is a bezier control point
                let next_point = &ring.points[i + 1];
                path_data.push_str(&format!(
                    " Q{},{} {},{}",
                    point.p.x.0,
                    page_height - point.p.y.0,
                    next_point.p.x.0,
                    page_height - next_point.p.y.0
                ));
                i += 2; // Skip the next point as it's the end of the bezier
            } else {
                // Regular line segment
                path_data.push_str(&format!(" L{},{}", point.p.x.0, page_height - point.p.y.0));
                i += 1;
            }
        }

        // Close the path for this ring
        path_data.push_str(" Z");
    }

    // Get styling based on PaintMode
    let (fill, stroke) = match polygon.mode {
        PaintMode::Fill => (color_to_svg(&gst.get_fill_color()), "none".to_string()),
        PaintMode::Stroke => ("none".to_string(), color_to_svg(&gst.get_stroke_color())),
        PaintMode::FillStroke => (
            color_to_svg(&gst.get_fill_color()),
            color_to_svg(&gst.get_stroke_color()),
        ),
        PaintMode::Clip => ("none".to_string(), "none".to_string()), /* Clip is handled
                                                                      * differently in SVG */
    };

    let stroke_width = gst.get_stroke_width().0;
    let line_join = gst.get_line_join().to_svg_string();
    let line_cap = gst.get_line_cap().get_svg_id();

    // Convert PDF winding order to SVG fill-rule
    let fill_rule = match polygon.winding_order {
        WindingOrder::NonZero => "nonzero",
        WindingOrder::EvenOdd => "evenodd",
    };

    // Handle dash pattern if present
    let dash_array = match gst.get_dash_array() {
        Some(dash) => {
            let dash_array = dash.as_array();
            if dash_array.is_empty() {
                String::new()
            } else {
                format!(
                    " stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\"",
                    dash_array
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                    dash.offset
                )
            }
        }
        None => String::new(),
    };

    // Get transformation if present
    let transform = get_svg_transform(
        &gst.get_transform_matrix(),
        &None, // Polygon doesn't use text matrix
        page_height,
    );

    let transform_attr = if !transform.is_empty() {
        format!(" transform=\"{}\"", transform)
    } else {
        String::new()
    };

    // Create the SVG path element
    format!(
        "<path d=\"{}\" fill=\"{}\" fill-rule=\"{}\" stroke=\"{}\" stroke-width=\"{}\" \
         stroke-linejoin=\"{}\" stroke-linecap=\"{}\"{}{} />",
        path_data,
        fill,
        fill_rule,
        stroke,
        stroke_width,
        line_join,
        line_cap,
        dash_array,
        transform_attr
    )
}

// Renders an image to SVG
fn render_image_to_svg(
    id: &XObjectId,
    transform: &XObjectTransform,
    resources: &PdfResources,
    image_map: &BTreeMap<XObjectId, (Vec<u8>, OutputImageFormat)>,
    page_height: f32,
    gst: &GraphicsStateVec,
) -> String {
    // Get the XObject and its binary data if available
    let xobject_opt = resources.xobjects.map.get(id);
    let image_data_opt = image_map.get(id);

    if let (Some(xobject), Some((bytes, fmt))) = (xobject_opt, image_data_opt) {
        if let Some((width, height)) = xobject.get_width_height() {
            // Convert dimensions to points
            let dpi = transform.dpi.unwrap_or(300.0);
            let w_pt = width.into_pt(dpi).0;
            let h_pt = height.into_pt(dpi).0;

            // Base64 encode the image data
            let base64_str = base64::prelude::BASE64_STANDARD.encode(bytes);
            let data_url = format!("data:{};base64,{}", fmt.mime_type(), base64_str);

            // Calculate transformations
            // Start with the XObject transform
            let mut transforms = Vec::new();

            // Apply scaling if specified
            if let Some(scale_x) = transform.scale_x {
                let scale_y = transform.scale_y.unwrap_or(scale_x);
                transforms.push(format!("scale({}, {})", scale_x, scale_y));
            }

            // Apply rotation if specified
            if let Some(rotate) = &transform.rotate {
                transforms.push(format!(
                    "rotate({}, {}, {})",
                    rotate.angle_ccw_degrees,
                    rotate.rotation_center_x.into_pt(dpi).0,
                    page_height - rotate.rotation_center_y.into_pt(dpi).0
                ));
            }

            // Apply translation if specified
            let tx = transform.translate_x.unwrap_or(Pt(0.0));
            let ty = transform.translate_y.unwrap_or(Pt(0.0));
            transforms.push(format!("translate({}, {})", tx.0, page_height - ty.0));

            // Apply current transformation matrix from graphics state
            let ctm = gst.get_transform_matrix();
            if ctm != CurTransMat::Identity {
                transforms.push(ctm.as_css_val());
            }

            let transform_attr = if !transforms.is_empty() {
                format!(" transform=\"{}\"", transforms.join(" "))
            } else {
                String::new()
            };

            // Create the SVG image element
            return format!(
                "<image width=\"{}\" height=\"{}\" href=\"{}\"{} />",
                w_pt, h_pt, data_url, transform_attr
            );
        }
    }

    String::new() // Return empty string if something failed
}
