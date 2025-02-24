use crate::ops::PdfPage;
use crate::serialize::prepare_fonts;
use crate::{OutputImageFormat, PdfResources};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PdfToSvgOptions {
    /// When rendering ImageXObjects, the images are embedded in the SVG.
    /// You can specify here, which image formats you'd like to output, i.e.
    /// `[Jpeg, Png, Avif]` will first try to encode the image to
    /// `image/jpeg,base64=...`, if the encoding fails, it will try
    /// `Png`, and last `Avif`. This is because if you want to render the SVG later on
    /// using `svg2png`, not all image formats might be supported, but generally in a
    /// browser context you can use `WebP` and `Avif` to save space.
    pub output_image_formats: Vec<OutputImageFormat>,
}

impl Default for PdfToSvgOptions {
    fn default() -> Self {
        Self {
            output_image_formats: vec![
                OutputImageFormat::Png,
                OutputImageFormat::Jpeg,
                OutputImageFormat::Bmp,
            ],
        }
    }
}

impl PdfToSvgOptions {
    pub fn web() -> Self {
        Self {
            output_image_formats: vec![
                OutputImageFormat::Avif,
                OutputImageFormat::WebP,
                OutputImageFormat::Jpeg,
                OutputImageFormat::Png,
                OutputImageFormat::Bmp,
                OutputImageFormat::Tiff,
                OutputImageFormat::Gif,
                OutputImageFormat::Tga,
            ],
        }
    }
}

pub fn render_to_svg(page: &PdfPage, resources: &PdfResources, opts: &PdfToSvgOptions) -> String {
    // Extract page dimensions from the media box.
    let width = page.media_box.width.0;
    let height = page.media_box.height.0;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = width,
        h = height
    ));
    svg.push('\n');

    // Embed fonts via a <style> block.
    svg.push_str("<style>\n");
    // Iterate over PDF fonts and embed each via an @font-face rule.
    for (font_id, font) in prepare_fonts(resources, &[page.clone()]).iter() {
        svg.push_str(&format!(
            r#"@font-face {{ font-family: "{}"; src: url("data:font/otf;charset=utf-8;base64,{}"); }}"#,
            font_id.0, base64::encode(&font.subset_font.bytes),
        ));
    }
    svg.push_str("</style>\n");

    // Initialize rendering state.
    let mut current_x = 0.0;
    let mut current_y = height;
    let mut current_transform = String::new();
    let mut current_font: Option<String> = None;
    let mut current_font_size: f32 = 12.0;
    let mut current_fill_color = String::from("black");
    let mut current_stroke_color = String::from("none");
    let mut current_stroke_width: f32 = 1.0;
    let mut current_dash_array: Option<String> = None;
    let mut current_line_join: Option<String> = None;
    let mut current_line_cap: Option<String> = None;
    let mut current_miter_limit: Option<f32> = None;
    let mut current_letter_spacing: f32 = 0.0;
    let mut current_line_offset: f32 = 0.0;
    let mut current_text_rendering_mode: Option<String> = None;

    // Process each PDF operation.
    for op in &page.ops {
        match op {
            // Position the text cursor.
            crate::ops::Op::SetTextCursor { pos } => {
                current_x = pos.x.0;
                current_y = pos.y.0;
            }
            // Set the current transformation matrix.
            crate::ops::Op::SetTransformationMatrix { matrix } => {
                // Assume matrix.as_array() returns [a, b, c, d, e, f].
                let m = matrix.as_array();
                current_transform = format!(
                    "matrix({} {} {} {} {} {})",
                    m[0], m[1], m[2], m[3], m[4], m[5]
                );
            }
            // Set fill color.
            crate::ops::Op::SetFillColor { col } => {
                use crate::color::Color;
                if let Color::Rgb(rgb) = col {
                    current_fill_color = format!(
                        "rgb({},{},{})",
                        (rgb.r * 255.0) as u8,
                        (rgb.g * 255.0) as u8,
                        (rgb.b * 255.0) as u8
                    );
                }
            }
            // Set outline (stroke) color.
            crate::ops::Op::SetOutlineColor { col } => {
                use crate::color::Color;
                if let Color::Rgb(rgb) = col {
                    current_stroke_color = format!(
                        "rgb({},{},{})",
                        (rgb.r * 255.0) as u8,
                        (rgb.g * 255.0) as u8,
                        (rgb.b * 255.0) as u8
                    );
                }
            }
            // Set outline thickness.
            crate::ops::Op::SetOutlineThickness { pt } => {
                current_stroke_width = pt.0;
            }
            // Set line dash pattern.
            crate::ops::Op::SetLineDashPattern { dash } => {
                let dash_array = dash.as_array();
                current_dash_array = Some(
                    dash_array
                        .iter()
                        .map(|num| num.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            // Set line join style.
            crate::ops::Op::SetLineJoinStyle { join } => {
                use crate::graphics::LineJoinStyle;
                current_line_join = Some(
                    match join {
                        LineJoinStyle::Miter => "miter",
                        LineJoinStyle::Round => "round",
                        LineJoinStyle::Bevel => "bevel",
                    }
                    .to_string(),
                );
            }
            // Set line cap style.
            crate::ops::Op::SetLineCapStyle { cap } => {
                use crate::graphics::LineCapStyle;
                current_line_cap = Some(
                    match cap {
                        LineCapStyle::Butt => "butt",
                        LineCapStyle::Round => "round",
                        LineCapStyle::ProjectingSquare => "square",
                    }
                    .to_string(),
                );
            }
            // Set miter limit.
            crate::ops::Op::SetMiterLimit { limit } => {
                current_miter_limit = Some(limit.0);
            }
            // Set text rendering mode.
            crate::ops::Op::SetTextRenderingMode { mode } => {
                current_text_rendering_mode = Some(format!("{:?}", mode));
            }
            // Set character spacing.
            crate::ops::Op::SetCharacterSpacing { multiplier } => {
                current_letter_spacing = *multiplier;
            }
            // Set line offset.
            crate::ops::Op::SetLineOffset { multiplier } => {
                current_line_offset = *multiplier;
            }
            // Render text using an external font.
            crate::ops::Op::WriteText { text, size, font } => {
                current_font = Some(font.0.clone());
                current_font_size = size.0;
                svg.push_str(&format!(
                    r#"<text x="{x}" y="{y}" font-family="{font}" font-size="{size}" fill="{fill}" transform="{transform}" letter-spacing="{ls}" baseline-shift="{lo}">{text}</text>"#,
                    x = current_x,
                    y = current_y,
                    font = current_font.as_deref().unwrap_or("sans-serif"),
                    size = current_font_size,
                    fill = current_fill_color,
                    transform = current_transform,
                    ls = current_letter_spacing,
                    lo = current_line_offset,
                    text = text
                ));
                svg.push('\n');
            }
            // Render text using a built-in font.
            crate::ops::Op::WriteTextBuiltinFont { text, size, font } => {
                current_font = Some(font.get_svg_font_family().to_string());
                let font_weight = font.get_font_weight();
                let font_style = font.get_font_style();
                current_font_size = size.0;
                svg.push_str(&format!(
                    r#"<text x="{x}" y="{y}" font-family="{font}" font-size="{size}" font-weight="{fw}" font-style="{fs}" fill="{fill}" transform="{transform}" letter-spacing="{ls}" baseline-shift="{lo}">{text}</text>"#,
                    x = current_x,
                    y = current_y,
                    font = current_font.as_deref().unwrap_or("sans-serif"),
                    size = current_font_size,
                    fw = font_weight,
                    fs = font_style,
                    fill = current_fill_color,
                    transform = current_transform,
                    ls = current_letter_spacing,
                    lo = current_line_offset,
                    text = text
                ));
                svg.push('\n');
            }
            // Draw a line.
            crate::ops::Op::DrawLine { line } => {
                let points: Vec<String> = line
                    .points
                    .iter()
                    .map(|(pt, _)| format!("{},{}", pt.x.0, pt.y.0))
                    .collect();
                let points_str = points.join(" ");
                if line.is_closed {
                    svg.push_str(&format!(
                        r#"<polygon points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" {dash} {join} {cap} {miter} transform="{transform}"/>"#,
                        points_str,
                        stroke = current_stroke_color,
                        sw = current_stroke_width,
                        dash = if let Some(ref d) = current_dash_array { format!(r#"stroke-dasharray="{}""#, d) } else { "".to_string() },
                        join = if let Some(ref j) = current_line_join { format!(r#"stroke-linejoin="{}""#, j) } else { "".to_string() },
                        cap = if let Some(ref c) = current_line_cap { format!(r#"stroke-linecap="{}""#, c) } else { "".to_string() },
                        miter = if let Some(ml) = current_miter_limit { format!(r#"stroke-miterlimit="{}""#, ml) } else { "".to_string() },
                        transform = current_transform
                    ));
                } else {
                    svg.push_str(&format!(
                        r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" {dash} {join} {cap} {miter} transform="{transform}"/>"#,
                        points_str,
                        stroke = current_stroke_color,
                        sw = current_stroke_width,
                        dash = if let Some(ref d) = current_dash_array { format!(r#"stroke-dasharray="{}""#, d) } else { "".to_string() },
                        join = if let Some(ref j) = current_line_join { format!(r#"stroke-linejoin="{}""#, j) } else { "".to_string() },
                        cap = if let Some(ref c) = current_line_cap { format!(r#"stroke-linecap="{}""#, c) } else { "".to_string() },
                        miter = if let Some(ml) = current_miter_limit { format!(r#"stroke-miterlimit="{}""#, ml) } else { "".to_string() },
                        transform = current_transform
                    ));
                }
                svg.push('\n');
            }
            // Draw a polygon.
            crate::ops::Op::DrawPolygon { polygon } => {
                for ring in &polygon.rings {
                    if let Some((first_pt, _)) = ring.first() {
                        let mut d = format!("M {} {}", first_pt.x.0, first_pt.y.0);
                        for (pt, _) in &ring[1..] {
                            d.push_str(&format!(" L {} {}", pt.x.0, pt.y.0));
                        }
                        if polygon.mode == crate::graphics::PaintMode::Fill
                            || polygon.mode == crate::graphics::PaintMode::FillStroke
                        {
                            d.push_str(" Z");
                        }
                        svg.push_str(&format!(
                            r#"<path d="{}" fill="{}" stroke="{stroke}" stroke-width="{sw}" {dash} {join} {cap} {miter} transform="{transform}"/>"#,
                            d,
                            current_fill_color,
                            stroke = current_stroke_color,
                            sw = current_stroke_width,
                            dash = if let Some(ref d) = current_dash_array { format!(r#"stroke-dasharray="{}""#, d) } else { "".to_string() },
                            join = if let Some(ref j) = current_line_join { format!(r#"stroke-linejoin="{}""#, j) } else { "".to_string() },
                            cap = if let Some(ref c) = current_line_cap { format!(r#"stroke-linecap="{}""#, c) } else { "".to_string() },
                            miter = if let Some(ml) = current_miter_limit { format!(r#"stroke-miterlimit="{}""#, ml) } else { "".to_string() },
                            transform = current_transform
                        ));
                        svg.push('\n');
                    }
                }
            }
            // Render an image XObject.
            crate::ops::Op::UseXObject {
                id,
                transform: xform,
            } => {
                if let Some(xobj) = resources.xobjects.map.get(id) {
                    // For this example, assume xobj is of the image variant.
                    if let crate::xobject::XObject::Image(raw_image) = xobj {
                        let img_width = raw_image.width;
                        let img_height = raw_image.height;
                        match raw_image.encode_to_bytes(&opts.output_image_formats) {
                            Ok((encoded_bytes, fmt)) => {
                                let mime = fmt.mime_type();
                                let image_data = base64::encode(&encoded_bytes);
                                svg.push_str(&format!(
                                    r#"<image x="{}" y="{}" width="{}" height="{}" xlink:href="data:{};base64,{}" transform="{}"/>"#,
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
                                    r#"<div x="{}" y="{}" data-failed-svg="{}" data-encode-fail-cause="{e}" width="{}" height="{}" transform="{}"/>"#,
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
            _ => {}
        }
    }

    svg.push_str("</svg>");
    svg
}
