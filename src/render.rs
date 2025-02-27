use base64::Engine;
use serde_derive::{Deserialize, Serialize};
use svg2pdf::usvg::{Fill, Text};

use crate::{
    ChangedField, Color, CurTransMat, ExtendedGraphicsState, LineCapStyle, LineDashPattern,
    LineJoinStyle, OutputImageFormat, PdfResources, Point, Pt, RenderingIntent, TextMatrix,
    TextRenderingMode, ops::PdfPage, serialize::prepare_fonts,
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
    transform_matrix: Option<CurTransMat>,
    text_matrix: Option<TextMatrix>,
    fill_color: Option<Color>,
    stroke_color: Option<Color>,
    stroke_width: Option<Pt>,
    dash_array: Option<LineDashPattern>,
    line_join: Option<LineJoinStyle>,
    line_cap: Option<LineCapStyle>,
    character_spacing: Option<f32>,
    letter_spacing: Option<f32>,
    line_offset: Option<f32>,
    miter_limit: Option<Pt>,
    horizontal_scaling: Option<f32>,
    text_leading: Option<Pt>,
    rendering_intent: Option<RenderingIntent>,
    text_rendering_mode: Option<TextRenderingMode>,
    marked_content_stack: Vec<String>,
    in_compatibility_section: bool,
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

    let mut gs = GraphicsStateVec::new();

    for op in &page.ops {
        match op {
            Op::SetRenderingIntent { intent } => {
                gs.set_rendering_intent(*intent);
            }
            Op::SetHorizontalScaling { percent } => {
                gs.set_horizontal_scaling(*percent);
            }
            Op::AddLineBreak => {
                // TODO: move text cursor ??
            }
            // Text “show” shortcut operators
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
            // Inline image operators (you may choose to handle these specially)
            Op::BeginInlineImage => {
                // (For now, possibly note that an inline image is beginning.)
            }
            Op::BeginInlineImageData => { /* … */ }
            Op::EndInlineImage => { /* … */ }
            Op::SetLineOffset { multiplier } => {
                gs.set_line_offset(*multiplier);
            }
            Op::SetLineHeight { lh } => {
                gs.set_line_height(*lh);
            }
            Op::SetTextCursor { pos } => {
                gs.set_text_cursor(*pos);
            }
            Op::SetTransformationMatrix { matrix } => {
                gs.set_cur_trans_mat(matrix.clone());
            }
            Op::SetTextMatrix { matrix } => {
                gs.set_text_mat(matrix.clone());
            }
            Op::SetFillColor { col } => {
                gs.set_fill_color(col.clone());
            }
            Op::SetOutlineColor { col } => {
                gs.set_outline_color(col.clone());
            }
            Op::SetOutlineThickness { pt } => {
                gs.set_stroke_width(*pt);
            }
            Op::SetLineDashPattern { dash } => {
                gs.set_dash_pattern(dash.clone());
            }
            Op::SetLineJoinStyle { join } => {
                gs.set_line_join(*join);
            }
            Op::SetLineCapStyle { cap } => {
                gs.set_line_cap(*cap);
            }
            Op::SetMiterLimit { limit } => {
                gs.set_miter_limit(*limit);
            }
            Op::SetTextRenderingMode { mode } => {
                gs.set_text_rendering_mode(*mode);
            }
            Op::SetCharacterSpacing { multiplier } => {
                gs.set_character_spacing(*multiplier);
            }
            Op::BeginMarkedContent { tag } => {
                gs.begin_marked_content(tag.clone());
            }
            Op::BeginMarkedContentWithProperties { tag, properties: _ } => {
                gs.begin_marked_content(tag.clone());
            }
            Op::DefineMarkedContentPoint { tag, properties: _ } => {
                gs.begin_marked_content(tag.clone());
            }
            Op::EndMarkedContent => {
                gs.end_marked_content();
            }
            Op::BeginCompatibilitySection => {
                gs.begin_compatibility_section();
            }
            Op::EndCompatibilitySection => {
                gs.end_compatibility_section();
            }

            /*
            // Render text using an external font.
            Op::WriteText { text, size, font } => {
                svg.push_str(&format!(
                    r#"<text x="{x}px" y="{y}px" font-family="{font}" font-size="{size}px" fill="{fill}" transform="{transform}" letter-spacing="{ls}" baseline-shift="{lo}">{text}</text>"#,
                    x = current_x,
                    y = height - current_y,
                    font = font.0,
                    size = size.0,
                    fill = current_fill_color,
                    transform = current_transform.as_css_val(true),
                    ls = current_letter_spacing,
                    lo = current_line_offset,
                    text = text
                ));
                svg.push('\n');
            }
            Op::WriteTextBuiltinFont { text, size, font } => {
                let font_weight = font.get_font_weight();
                let font_style = font.get_font_style();
                svg.push_str(&format!(
                    r#"<text x="{x}px" y="{y}px" font-family="{font}" font-size="{size}px" font-weight="{fw}" font-style="{fs}" fill="{fill}" transform="{transform}" letter-spacing="{ls}" baseline-shift="{lo}">{text}</text>"#,
                    x = current_x,
                    y = height - current_y,
                    font = font.get_svg_font_family(),
                    size = size.0,
                    fw = font_weight,
                    fs = font_style,
                    fill = current_fill_color,
                    transform = current_transform.as_css_val(true),
                    ls = current_letter_spacing,
                    lo = current_line_offset,
                    text = text
                ));
                svg.push('\n');
            }
            Op::DrawLine { line } => {
                let points: Vec<String> = line
                    .points
                    .iter()
                    .map(|pt| format!("{},{}", pt.p.x.0, pt.p.y.0))
                    .collect();
                let points_str = points.join(" ");
                if line.is_closed {
                    svg.push_str(&format!(
                        r#"<polygon points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" {dash} {join} {cap} {miter} />"#,
                        points_str,
                        stroke = current_stroke_color,
                        sw = current_stroke_width,
                        dash = if let Some(ref d) = current_dash_array { format!(r#"stroke-dasharray="{}""#, d) } else { "".to_string() },
                        join = if let Some(ref j) = current_line_join { format!(r#"stroke-linejoin="{}""#, j) } else { "".to_string() },
                        cap = if let Some(ref c) = current_line_cap { format!(r#"stroke-linecap="{}""#, c) } else { "".to_string() },
                        miter = if let Some(ml) = current_miter_limit { format!(r#"stroke-miterlimit="{}""#, ml) } else { "".to_string() },
                    ));
                } else {
                    svg.push_str(&format!(
                        r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" {dash} {join} {cap} {miter} />"#,
                        points_str,
                        stroke = current_stroke_color,
                        sw = current_stroke_width,
                        dash = if let Some(ref d) = current_dash_array { format!(r#"stroke-dasharray="{}""#, d) } else { "".to_string() },
                        join = if let Some(ref j) = current_line_join { format!(r#"stroke-linejoin="{}""#, j) } else { "".to_string() },
                        cap = if let Some(ref c) = current_line_cap { format!(r#"stroke-linecap="{}""#, c) } else { "".to_string() },
                        miter = if let Some(ml) = current_miter_limit { format!(r#"stroke-miterlimit="{}""#, ml) } else { "".to_string() },
                    ));
                }
                svg.push('\n');
            }
            Op::DrawPolygon { polygon } => {
                for ring in &polygon.rings {
                    if let Some(first_pt) = ring.points.first() {
                        let mut d = format!("M {} {}", first_pt.p.x.0, first_pt.p.y.0);
                        for pt in &ring.points[1..] {
                            d.push_str(&format!(" L {} {}", pt.p.x.0, pt.p.y.0));
                        }
                        if polygon.mode == crate::graphics::PaintMode::Fill
                            || polygon.mode == crate::graphics::PaintMode::FillStroke
                        {
                            d.push_str(" Z");
                        }
                        svg.push_str(&format!(
                            r#"<path d="{}" fill="{}" stroke="{stroke}" stroke-width="{sw}" {dash} {join} {cap} {miter} />"#,
                            d,
                            current_fill_color,
                            stroke = current_stroke_color,
                            sw = current_stroke_width,
                            dash = if let Some(ref d) = current_dash_array { format!(r#"stroke-dasharray="{}""#, d) } else { "".to_string() },
                            join = if let Some(ref j) = current_line_join { format!(r#"stroke-linejoin="{}""#, j) } else { "".to_string() },
                            cap = if let Some(ref c) = current_line_cap { format!(r#"stroke-linecap="{}""#, c) } else { "".to_string() },
                            miter = if let Some(ml) = current_miter_limit { format!(r#"stroke-miterlimit="{}""#, ml) } else { "".to_string() },
                        ));
                        svg.push('\n');
                    }
                }
            }
            Op::UseXObject {
                id,
                transform: xform,
            } => {
                if let Some(xobj) = resources.xobjects.map.get(id) {
                    // For this example, assume xobj is of the image variant.
                    if let crate::xobject::XObject::Image(raw_image) = xobj {
                        let img_width = raw_image.width;
                        let img_height = raw_image.height;
                        match raw_image.encode_to_bytes(&opts.image_formats) {
                            Ok((encoded_bytes, fmt)) => {
                                let mime = fmt.mime_type();
                                let image_data =
                                    base64::prelude::BASE64_STANDARD.encode(&encoded_bytes);
                                svg.push_str(&format!(
                                    r#"<image x="{}px" y="{}px" width="{}px" height="{}px" xlink:href="data:{};base64,{}" transform="{}"/>"#,
                                    current_x,
                                    current_y,
                                    img_width,
                                    img_height,
                                    mime,
                                    image_data,
                                    xform.as_svg_transform()
                                ));
                                svg.push('\n');
                            }
                            Err(e) => {
                                svg.push_str(&format!(
                                    r#"<div x="{}px" y="{}px" data-failed-svg="{}" data-encode-fail-cause="{e}" width="{}px" height="{}px" transform="{}"/>"#,
                                    current_x,
                                    current_y,
                                    id.0,
                                    img_width,
                                    img_height,
                                    xform.as_svg_transform()
                                ));
                                svg.push('\n');
                            }
                        }
                    }
                }
            }
            */
            _ => {}
        }
    }

    svg.push_str("</svg>");
    svg
}
