//! Bridge module to translate azul-layout PDF operations to printpdf operations.
//!
//! This module converts the intermediate PDF representation from azul-layout
//! into printpdf's native Op enum, allowing us to leverage azul's layout engine
//! while using printpdf's PDF generation.
//!
//! IMPORTANT: This module now converts DisplayList directly to printpdf Ops,
//! bypassing the intermediate azul PdfOp representation. This allows us to
//! Important: This module uses positioned glyphs from azul's layout engine to
//! generate ShowText operations (with SetFont for font/size), which map 1:1 to PDF operators.
//! which is necessary for proper text shaping in complex scripts.

use std::collections::BTreeMap;

use azul_core::{
    geom::{LogicalRect, LogicalSize, LogicalPosition},
    resources::DecodedImage,
};
use azul_css::props::basic::ColorU;
use azul_layout::{
    solver3::display_list::{DisplayList, DisplayListItem},
    text3::cache::{FontManager, ParsedFontTrait, UnifiedLayout},
};

use crate::{Color, Mm, Op, Pt, Rgb, FontId, RawImage, XObjectId, XObjectTransform,
    ExtendedGraphicsState, ExtendedGraphicsStateId, XObject};
use crate::shading::{GradientStop, Shading, ShadingGeometry, ShadingId};
use azul_css::props::basic::{
    color::ColorOrSystem,
    geometry::{LayoutPoint, LayoutRect, LayoutSize},
};

/// Resolved `<img>` resources keyed by the `src` value that appeared in the HTML.
///
/// Each entry pairs the deterministic [`XObjectId`] under which the image is
/// registered on the [`crate::PdfDocument`] with the decoded [`RawImage`] (kept
/// so the bridge can read the natural pixel size when building the placement
/// transform).
pub type ResolvedImages = BTreeMap<String, (XObjectId, RawImage)>;

/// PDF resources synthesized by the bridge while translating a display list, to
/// be registered on the destination [`crate::PdfDocument`] before serialization.
///
/// The bridge produces a flat `Vec<Op>` and has no access to the document, but
/// several display-list features need *document-level* resources referenced by
/// name from the content stream: alpha/opacity and soft masks need an
/// `ExtGState`, and rasterized effects (conic gradients, blurred shadows) need
/// image/form `XObject`s. The bridge mints a unique id (via `*::new()`) for each,
/// emits the op that references it, and records the definition here;
/// [`BridgeResources::register_into`] then drains them onto the document's
/// `resources` (mirroring how `<img>` Image XObjects are registered).
#[derive(Default)]
pub struct BridgeResources {
    /// Extended graphics states (fill/stroke alpha, soft masks).
    pub extgstates: Vec<(ExtendedGraphicsStateId, ExtendedGraphicsState)>,
    /// Axial/radial shadings (linear/radial gradients).
    pub shadings: Vec<(ShadingId, Shading)>,
    /// Image / form XObjects synthesized by the bridge.
    pub xobjects: Vec<(XObjectId, XObject)>,
}

impl BridgeResources {
    /// Allocate an `ExtGState`, record it, and return its id (for `LoadGraphicsState`).
    fn add_extgstate(&mut self, gs: ExtendedGraphicsState) -> ExtendedGraphicsStateId {
        let id = ExtendedGraphicsStateId::new();
        self.extgstates.push((id.clone(), gs));
        id
    }

    /// Allocate a shading, record it, and return its id (for `PaintShading`).
    fn add_shading(&mut self, shading: Shading) -> ShadingId {
        let id = ShadingId::new();
        self.shadings.push((id.clone(), shading));
        id
    }

    /// Drain the synthesized resources into a document's resource maps. Existing
    /// entries are kept (`or_insert`), matching the merge semantics elsewhere.
    pub fn register_into(self, resources: &mut crate::PdfResources) {
        for (id, gs) in self.extgstates {
            resources.extgstates.map.entry(id).or_insert(gs);
        }
        for (id, sh) in self.shadings {
            resources.shadings.map.entry(id).or_insert(sh);
        }
        for (id, xobj) in self.xobjects {
            resources.xobjects.map.entry(id).or_insert(xobj);
        }
    }
}

/// Build the deterministic [`XObjectId`] used to register an `<img src=KEY>`
/// image on the document. Both the bridge (which emits `Op::UseXobject`) and the
/// document-assembly step (which registers the XObject) derive the id from the
/// `src` key, so they always agree without sharing state.
pub fn image_xobject_id(src_key: &str) -> XObjectId {
    // Keep it readable but avoid characters that would be awkward in a PDF name.
    let sanitized: String = src_key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    XObjectId(format!("HtmlImg_{}", sanitized))
}

/// Decode every entry of an `images` map (key = `src`, value = raw image bytes)
/// into a [`ResolvedImages`] table. Images that fail to decode are skipped (the
/// corresponding `<img>` simply renders nothing).
pub fn resolve_html_images(images: &BTreeMap<String, Vec<u8>>) -> ResolvedImages {
    let mut out = ResolvedImages::new();
    for (key, bytes) in images.iter() {
        let mut warnings = Vec::new();
        if let Ok(raw) = RawImage::decode_from_bytes(bytes, &mut warnings) {
            out.insert(key.clone(), (image_xobject_id(key), raw));
        }
    }
    out
}

use super::border::{
    BorderConfig, extract_border_widths, extract_border_colors, 
    extract_border_styles, extract_border_radii, render_border,
};

/// Convert azul ColorU to printpdf Color
fn convert_color(color: &ColorU) -> Color {
    Color::Rgb(Rgb {
        r: color.r as f32 / 255.0,
        g: color.g as f32 / 255.0,
        b: color.b as f32 / 255.0,
        icc_profile: None,
    })
}

/// If `color` is translucent (`a < 255`), mint an `ExtGState` carrying the
/// fill+stroke alpha, record it on `bridge_res`, and emit `LoadGraphicsState`
/// so the subsequent fill/stroke draws with that alpha. Call this *inside* a
/// q/Q (Save/RestoreGraphicsState) scope so the alpha stays local to the
/// current primitive. No-op for fully opaque colors.
fn apply_fill_alpha(ops: &mut Vec<Op>, bridge_res: &mut BridgeResources, color: &ColorU) {
    if color.a >= 255 {
        return;
    }
    let alpha = color.a as f32 / 255.0;
    let mut gs = ExtendedGraphicsState::default();
    gs.set_current_fill_alpha(alpha);
    gs.set_current_stroke_alpha(alpha);
    let id = bridge_res.add_extgstate(gs);
    ops.push(Op::LoadGraphicsState { gs: id });
}

/// Build a clip-mode polygon (for the `W n` clip operator) covering `bounds`
/// (CSS px) with optional rounded corners `radii_px` = `[tl, tr, br, bl]` in
/// CSS px. Coordinates are converted to PDF pt (with the Y-axis flip). Used to
/// constrain a gradient `sh` paint (and box backgrounds) to the element box.
fn make_clip_polygon(
    transform: &CoordTransform,
    bounds: &LogicalRect,
    radii_px: [f32; 4],
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
) -> crate::graphics::Polygon {
    let radii = crate::html::border::BorderRadii {
        top_left: (radii_px[0] * CSS_PX_TO_PT, radii_px[0] * CSS_PX_TO_PT),
        top_right: (radii_px[1] * CSS_PX_TO_PT, radii_px[1] * CSS_PX_TO_PT),
        bottom_right: (radii_px[2] * CSS_PX_TO_PT, radii_px[2] * CSS_PX_TO_PT),
        bottom_left: (radii_px[3] * CSS_PX_TO_PT, radii_px[3] * CSS_PX_TO_PT),
    };
    if radii_px.iter().any(|r| *r > 0.0) {
        let b = bounds_px_to_pt(bounds);
        let points = crate::html::border::create_rounded_rect_path_with_margins(
            b.origin.x, b.origin.y, b.size.width, b.size.height,
            &radii, page_height, margin_left, margin_top,
        );
        crate::graphics::Polygon {
            rings: vec![crate::graphics::PolygonRing { points }],
            mode: crate::graphics::PaintMode::Clip,
            winding_order: crate::graphics::WindingOrder::NonZero,
        }
    } else {
        let x = transform.x(bounds.origin.x);
        let y = transform.rect_y(bounds.origin.y, bounds.size.height);
        let w = transform.dim(bounds.size.width);
        let h = transform.dim(bounds.size.height);
        let mut p = make_rect_polygon_pt(x, y, w, h);
        p.mode = crate::graphics::PaintMode::Clip;
        p
    }
}

/// Resolve a gradient stop color to a concrete RGBA. System colors are
/// theme-dependent; for static PDF output they fall back to opaque black
/// (rare in document gradients).
fn resolve_stop_color(c: &ColorOrSystem) -> ColorU {
    match c {
        ColorOrSystem::Color(col) => *col,
        ColorOrSystem::System(_) => ColorU { r: 0, g: 0, b: 0, a: 255 },
    }
}

/// Convert azul normalized linear color stops (offset 0..100%) to shading stops
/// (offset 0..1, RGB 0..1). Per-stop alpha is dropped — PDF axial/radial
/// shadings have no alpha channel (a translucent gradient would need a soft mask).
fn normalize_gradient_stops(
    stops: &azul_css::props::style::background::NormalizedLinearColorStopVec,
) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|s| {
            let c = resolve_stop_color(&s.color);
            GradientStop {
                offset: s.offset.normalized().clamp(0.0, 1.0),
                color: [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0],
            }
        })
        .collect()
}

/// Convert a display list directly to printpdf Ops with margin support.
/// 
/// This version applies margins during coordinate transformation:
/// - `margin_left_pt`: Shifts all X coordinates to the right
/// - `margin_top_pt`: Shifts all Y coordinates down from the top
/// 
/// The layout engine produces coordinates relative to (0,0) at top-left of the content area.
/// PDF uses bottom-left origin. This function transforms coordinates as:
/// - PDF_X = layout_x + margin_left
/// - PDF_Y = page_height - layout_y - margin_top
pub fn display_list_to_printpdf_ops_with_margins<T: ParsedFontTrait + 'static>(
    display_list: &DisplayList,
    page_size: LogicalSize,
    margin_left_pt: f32,
    margin_top_pt: f32,
    font_manager: &FontManager<T>,
    images: &ResolvedImages,
    bridge_res: &mut BridgeResources,
) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    let page_height = page_size.height;
    
    // Track the current TextLayout for glyph-to-unicode mapping
    let mut current_text_layout: Option<(&azul_layout::text3::cache::UnifiedLayout, LogicalRect)> = None;

    let _text_layout_count = display_list.items.iter().filter(|item| matches!(item, DisplayListItem::TextLayout { .. })).count();

    for (_idx, item) in display_list.items.iter().enumerate() {
        convert_display_list_item_with_margins(
            &mut ops,
            item,
            page_height,
            margin_left_pt,
            margin_top_pt,
            &mut current_text_layout,
            font_manager,
            images,
            bridge_res,
        );
    }

    Ok(ops)
}

/// Convert a display list directly to printpdf Ops (without margins, for backwards compatibility).
/// This bypasses the intermediate azul PdfOp format to generate
/// WriteCodepoints has been replaced with ShowText (use SetFont first to set the font).
pub fn display_list_to_printpdf_ops<T: ParsedFontTrait + 'static>(
    display_list: &DisplayList,
    page_size: LogicalSize,
    font_manager: &FontManager<T>,
) -> Result<Vec<Op>, String> {
    // Back-compat entry point: callers here do not collect bridge-synthesized
    // resources (alpha/opacity/gradients/shadows), so those would not be
    // registered. Prefer `display_list_to_printpdf_ops_with_margins` +
    // `BridgeResources::register_into`.
    let mut bridge_res = BridgeResources::default();
    display_list_to_printpdf_ops_with_margins(display_list, page_size, 0.0, 0.0, font_manager, &ResolvedImages::new(), &mut bridge_res)
}

/// CSS px to PDF pt conversion factor.
/// The layout engine works in CSS px (where 1pt = 96/72 px).
/// PDF coordinates are in pt (1pt = 1/72 inch).
/// To convert layout px back to PDF pt: multiply by 72/96.
const CSS_PX_TO_PT: f32 = 72.0 / 96.0;

/// Convert a value in PDF pt to `Mm` (1 pt = 25.4/72 mm ≈ 0.352778 mm)
#[inline]
fn pt_to_mm(pt: f32) -> Mm {
    Mm::from(Pt(pt))
}

/// Convert a CSS-px `LogicalRect` to PDF-pt `LogicalRect`.
fn bounds_px_to_pt(b: &LogicalRect) -> LogicalRect {
    LogicalRect {
        origin: LogicalPosition::new(
            b.origin.x * CSS_PX_TO_PT,
            b.origin.y * CSS_PX_TO_PT,
        ),
        size: LogicalSize::new(
            b.size.width * CSS_PX_TO_PT,
            b.size.height * CSS_PX_TO_PT,
        ),
    }
}

/// Build a filled rectangle polygon from PDF-pt coordinates.
fn make_rect_polygon_pt(x: f32, y: f32, w: f32, h: f32) -> crate::graphics::Polygon {
    let lp = |px: f32, py: f32| crate::graphics::LinePoint {
        p: crate::graphics::Point::new(pt_to_mm(px), pt_to_mm(py)),
        bezier: false,
    };
    crate::graphics::Polygon {
        rings: vec![crate::graphics::PolygonRing {
            points: vec![
                lp(x, y),
                lp(x + w, y),
                lp(x + w, y + h),
                lp(x, y + h),
            ],
        }],
        mode: crate::graphics::PaintMode::Fill,
        winding_order: crate::graphics::WindingOrder::NonZero,
    }
}

/// Coordinate transformation parameters for converting from layout to PDF coordinates.
/// All layout coordinates are in CSS px; this struct converts them to PDF pt.
#[derive(Clone, Copy)]
struct CoordTransform {
    /// Full page height in PDF pt (for Y-axis flip)
    page_height: f32,
    /// Left margin in PDF pt
    margin_left: f32,
    /// Top margin in PDF pt
    margin_top: f32,
}

impl CoordTransform {
    fn new(page_height: f32, margin_left: f32, margin_top: f32) -> Self {
        Self { page_height, margin_left, margin_top }
    }
    
    /// Transform X coordinate from layout space (CSS px) to PDF space (pt)
    #[inline]
    fn x(&self, layout_x: f32) -> f32 {
        layout_x * CSS_PX_TO_PT + self.margin_left
    }
    
    /// Transform Y coordinate from layout space (CSS px) to PDF space (pt)
    /// Layout Y is from top (increases downward), PDF Y is from bottom (increases upward)
    #[inline]
    fn y(&self, layout_y: f32) -> f32 {
        self.page_height - layout_y * CSS_PX_TO_PT - self.margin_top
    }
    
    /// Transform a rectangle's Y position (accounts for rectangle height in CSS px)
    #[inline]
    fn rect_y(&self, layout_y: f32, height: f32) -> f32 {
        self.page_height - (layout_y + height) * CSS_PX_TO_PT - self.margin_top
    }
    
    /// Convert a dimension (width, height, font size) from CSS px to PDF pt
    #[inline]
    fn dim(&self, px_value: f32) -> f32 {
        px_value * CSS_PX_TO_PT
    }
}

fn convert_display_list_item_with_margins<'a, T: ParsedFontTrait + 'static>(
    ops: &mut Vec<Op>,
    item: &'a DisplayListItem,
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
    current_text_layout: &mut Option<(&'a UnifiedLayout, LogicalRect)>,
    font_manager: &FontManager<T>,
    images: &ResolvedImages,
    bridge_res: &mut BridgeResources,
) {
    let transform = CoordTransform::new(page_height, margin_left, margin_top);
    
    match item {
        DisplayListItem::Rect {
            bounds,
            color,
            border_radius,
        } => {
            // Skip rectangles with zero size (layout artifacts from empty inline elements)
            if bounds.size().width == 0.0 || bounds.size().height == 0.0 {
                return;
            }
            
            // Convert rectangle to PDF polygon
            ops.push(Op::SaveGraphicsState);
            apply_fill_alpha(ops, bridge_res, color);

            // Convert DisplayList BorderRadius (CSS px) to border-module BorderRadii,
            // scaled to PDF pt to match the pt-space `b` computed below.
            let radii = crate::html::border::BorderRadii {
                top_left: (border_radius.top_left * CSS_PX_TO_PT, border_radius.top_left * CSS_PX_TO_PT),
                top_right: (border_radius.top_right * CSS_PX_TO_PT, border_radius.top_right * CSS_PX_TO_PT),
                bottom_right: (border_radius.bottom_right * CSS_PX_TO_PT, border_radius.bottom_right * CSS_PX_TO_PT),
                bottom_left: (border_radius.bottom_left * CSS_PX_TO_PT, border_radius.bottom_left * CSS_PX_TO_PT),
            };
            
            // Check if we have border radius
            let has_radius = radii.top_left.0 > 0.0 || radii.top_left.1 > 0.0
                || radii.top_right.0 > 0.0 || radii.top_right.1 > 0.0
                || radii.bottom_right.0 > 0.0 || radii.bottom_right.1 > 0.0
                || radii.bottom_left.0 > 0.0 || radii.bottom_left.1 > 0.0;
            
            if has_radius {
                // Use rounded rectangle path for filling (with margin-adjusted coordinates)
                // Convert layout coordinates from CSS px to PDF pt before passing
                let b = bounds_px_to_pt(bounds.inner());
                let points = crate::html::border::create_rounded_rect_path_with_margins(
                    b.origin.x,
                    b.origin.y,
                    b.size.width,
                    b.size.height,
                    &radii,
                    page_height,
                    margin_left,
                    margin_top,
                );
                
                let polygon = crate::graphics::Polygon {
                    rings: vec![crate::graphics::PolygonRing { points }],
                    mode: crate::graphics::PaintMode::Fill,
                    winding_order: crate::graphics::WindingOrder::NonZero,
                };
                
                ops.push(Op::SetFillColor {
                    col: convert_color(color),
                });
                ops.push(Op::DrawPolygon { polygon });
            } else {
                // Simple rectangle without border radius
                let x = transform.x(bounds.origin().x);
                let y = transform.rect_y(bounds.origin().y, bounds.size().height);
                let w = transform.dim(bounds.size().width);
                let h = transform.dim(bounds.size().height);

                ops.push(Op::SetFillColor {
                    col: convert_color(color),
                });
                ops.push(Op::DrawPolygon { polygon: make_rect_polygon_pt(x, y, w, h) });
            }
            
            ops.push(Op::RestoreGraphicsState);
        }

        DisplayListItem::TextLayout {
            layout,
            bounds,
            font_hash: _,
            font_size_px: _,
            color,
        } => {
            // Extract the UnifiedLayout from the type-erased Arc<dyn Any>
            if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout>() {
                // Process this TextLayout immediately with margin support
                render_unified_layout_with_margins(ops, unified_layout, bounds.inner(), *color, &transform, font_manager);

                // Also update the current text layout for any subsequent processing
                *current_text_layout = Some((unified_layout, *bounds.inner()));
            }
        }

        DisplayListItem::Text { glyphs, font_hash, font_size_px, color, clip_rect: _, source_node_index: _ } => {
            // Render simple text items (used for headers/footers)
            // These use Unicode codepoints as glyph indices and a placeholder font hash (0)
            // 
            // IMPORTANT: For external fonts (font_hash != 0), the TextLayout item already
            // renders the text via render_unified_layout_with_margins(). The Text items
            // are kept in the display list for other renderers (like WebRender for screen)
            // but should be SKIPPED for PDF output to avoid double-rendering.
            if glyphs.is_empty() {
                return;
            }
            
            // For text with font_hash == 0, use a builtin PDF font (Helvetica)
            // This is used for page headers/footers which don't go through text shaping
            let use_builtin_font = font_hash.font_hash == 0;
            
            // Skip external fonts - they are rendered via TextLayout
            if !use_builtin_font {
                return;
            }
            
            ops.push(Op::SaveGraphicsState);
            
            // Set text color
            ops.push(Op::SetFillColor {
                col: convert_color(color),
            });
            
            if use_builtin_font {
                // Use Helvetica for system-generated text (headers/footers)
                ops.push(Op::SetFont {
                    font: crate::ops::PdfFontHandle::Builtin(crate::BuiltinFont::Helvetica),
                    size: Pt(transform.dim(*font_size_px)),
                });
            } else {
                // Use external font by hash
                let font_id = FontId(format!("F{}", font_hash.font_hash));
                ops.push(Op::SetFont {
                    font: crate::ops::PdfFontHandle::External(font_id),
                    size: Pt(transform.dim(*font_size_px)),
                });
            }
            
            // Start text section
            ops.push(Op::StartTextSection);
            
            // Render glyphs - collect all characters into a single text string for better rendering
            // Group glyphs by approximate y-position to handle potential line breaks
            let mut text_string = String::new();
            let mut first_glyph_pos: Option<(f32, f32)> = None;
            
            for glyph in glyphs {
                if first_glyph_pos.is_none() {
                    first_glyph_pos = Some((glyph.point.x, glyph.point.y));
                }
                
                // For builtin fonts, glyph.index is the Unicode codepoint
                if use_builtin_font {
                    if let Some(ch) = char::from_u32(glyph.index) {
                        text_string.push(ch);
                    }
                }
            }
            
            if let Some((x, y)) = first_glyph_pos {
                // Convert coordinates
                let pdf_x = transform.x(x);
                let pdf_y = transform.y(y);
                
                // Set position for this text
                ops.push(Op::SetTextMatrix {
                    matrix: crate::matrix::TextMatrix::Raw([
                        1.0, 0.0,   // No scaling/rotation
                        0.0, 1.0,
                        pdf_x, pdf_y,
                    ]),
                });
                
                if use_builtin_font && !text_string.is_empty() {
                    ops.push(Op::ShowText {
                        items: vec![crate::text::TextItem::Text(text_string)],
                    });
                } else if !use_builtin_font {
                    // For external fonts, use glyph IDs
                    let glyph_ids: Vec<crate::text::Codepoint> = glyphs.iter().map(|g| {
                        crate::text::Codepoint::new(g.index as u16, 0.0)
                    }).collect();
                    
                    ops.push(Op::ShowText {
                        items: vec![crate::text::TextItem::GlyphIds(glyph_ids)],
                    });
                }
            }
            
            // End text section
            ops.push(Op::EndTextSection);
            
            ops.push(Op::RestoreGraphicsState);
        }

        DisplayListItem::Border {
            bounds,
            widths,
            colors,
            styles,
            border_radius,
        } => {
            // Use comprehensive border rendering with margin support
            // Convert bounds from CSS px to PDF pt
            let bounds_pt = bounds_px_to_pt(bounds.inner());
            let config = BorderConfig {
                bounds: bounds_pt,
                widths: extract_border_widths(widths),
                colors: extract_border_colors(colors),
                styles: extract_border_styles(styles),
                radii: extract_border_radii(border_radius),
                page_height,
                margin_left,
                margin_top,
            };
            
            render_border(ops, &config);
        }

        DisplayListItem::Image { bounds, image, border_radius: _ } => {
            // The display-list `ImageRef` for an HTML `<img>` is a `NullImage`
            // whose `tag` carries the original `src` string (set by azul's
            // `xml_node_to_dom_fast`). We use that key to look up the decoded
            // bytes — already registered as a PDF Image XObject on the document —
            // and emit a `UseXobject` op positioned at `bounds`.
            let src_key = match image.get_data() {
                DecodedImage::NullImage { tag, .. } if !tag.is_empty() => {
                    String::from_utf8(tag.clone()).ok()
                }
                _ => None,
            };

            let Some(src_key) = src_key else { return; };
            let Some((xobject_id, raw_image)) = images.get(&src_key) else { return; };

            // Target placement rectangle in PDF pt.
            let b = bounds_px_to_pt(bounds.inner());
            let target_w_pt = b.size.width;
            let target_h_pt = b.size.height;
            if target_w_pt <= 0.0 || target_h_pt <= 0.0 {
                return;
            }

            // The serializer maps the image's unit square to its natural size in
            // pt at `IMAGE_DPI` first (see `XObjectTransform::get_ctms`), so we
            // scale by target/natural to make the image fill `bounds` exactly.
            const IMAGE_DPI: f32 = 300.0;
            let natural_w_pt = crate::Px(raw_image.width).into_pt(IMAGE_DPI).0;
            let natural_h_pt = crate::Px(raw_image.height).into_pt(IMAGE_DPI).0;
            if natural_w_pt <= 0.0 || natural_h_pt <= 0.0 {
                return;
            }
            let scale_x = target_w_pt / natural_w_pt;
            let scale_y = target_h_pt / natural_h_pt;

            // Bottom-left corner of the image box in PDF coordinates (origin
            // bottom-left). `transform.rect_y` already accounts for the rect
            // height and top margin; `transform.x` adds the left margin.
            let pdf_x = transform.x(bounds.origin().x);
            let pdf_y = transform.rect_y(bounds.origin().y, bounds.size().height);

            ops.push(Op::UseXobject {
                id: xobject_id.clone(),
                transform: XObjectTransform {
                    translate_x: Some(Pt(pdf_x)),
                    translate_y: Some(Pt(pdf_y)),
                    rotate: None,
                    scale_x: Some(scale_x),
                    scale_y: Some(scale_y),
                    dpi: Some(IMAGE_DPI),
                },
            });
        }

        DisplayListItem::Underline { bounds, color, thickness: _ }
        | DisplayListItem::Strikethrough { bounds, color, thickness: _ }
        | DisplayListItem::Overline { bounds, color, thickness: _ } => {
            // Text decorations are rendered as simple filled rectangles.
            // The decoration thickness is already encoded in bounds.size.height.
            if bounds.size().width > 0.0 && bounds.size().height > 0.0 {
                let x = transform.x(bounds.origin().x);
                let y = transform.rect_y(bounds.origin().y, bounds.size().height);
                let w = transform.dim(bounds.size().width);
                let h = transform.dim(bounds.size().height);

                ops.push(Op::SaveGraphicsState);
                apply_fill_alpha(ops, bridge_res, color);
                ops.push(Op::SetFillColor { col: convert_color(color) });
                ops.push(Op::DrawPolygon { polygon: make_rect_polygon_pt(x, y, w, h) });
                ops.push(Op::RestoreGraphicsState);
            }
        }

        DisplayListItem::LinearGradient { bounds, gradient, border_radius } => {
            let inner = bounds.inner();
            if inner.size.width <= 0.0 || inner.size.height <= 0.0 {
                return;
            }
            // Gradient-line endpoints in local px (azul's CSS direction math),
            // mapped to absolute PDF pt (the transform applies px->pt + Y-flip).
            let rect = LayoutRect::new(
                LayoutPoint::new(0, 0),
                LayoutSize::new(inner.size.width as isize, inner.size.height as isize),
            );
            let (p0, p1) = gradient.direction.to_points(&rect);
            let coords = [
                transform.x(inner.origin.x + p0.x as f32),
                transform.y(inner.origin.y + p0.y as f32),
                transform.x(inner.origin.x + p1.x as f32),
                transform.y(inner.origin.y + p1.y as f32),
            ];
            let stops = normalize_gradient_stops(&gradient.stops);
            if stops.is_empty() {
                return;
            }
            let id = bridge_res.add_shading(Shading {
                geometry: ShadingGeometry::Axial { coords },
                stops,
                extend: (true, true),
            });
            // Clip to the (optionally rounded) box, then paint the shading.
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::DrawPolygon {
                polygon: make_clip_polygon(
                    &transform, inner,
                    [border_radius.top_left, border_radius.top_right,
                     border_radius.bottom_right, border_radius.bottom_left],
                    page_height, margin_left, margin_top,
                ),
            });
            ops.push(Op::PaintShading { id });
            ops.push(Op::RestoreGraphicsState);
        }

        DisplayListItem::RadialGradient { bounds, gradient, border_radius } => {
            let inner = bounds.inner();
            if inner.size.width <= 0.0 || inner.size.height <= 0.0 {
                return;
            }
            // v1: center the gradient in the box and use the farthest-corner
            // radius (the CSS default). Explicit position/size keywords and
            // ellipse aspect ratios are approximated as a centered circle.
            let cx = inner.size.width / 2.0;
            let cy = inner.size.height / 2.0;
            let r_px = cx.max(inner.size.width - cx).hypot(cy.max(inner.size.height - cy));
            let center_x = transform.x(inner.origin.x + cx);
            let center_y = transform.y(inner.origin.y + cy);
            let r_pt = r_px * CSS_PX_TO_PT;
            let coords = [center_x, center_y, 0.0, center_x, center_y, r_pt];
            let stops = normalize_gradient_stops(&gradient.stops);
            if stops.is_empty() {
                return;
            }
            let id = bridge_res.add_shading(Shading {
                geometry: ShadingGeometry::Radial { coords },
                stops,
                extend: (true, true),
            });
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::DrawPolygon {
                polygon: make_clip_polygon(
                    &transform, inner,
                    [border_radius.top_left, border_radius.top_right,
                     border_radius.bottom_right, border_radius.bottom_left],
                    page_height, margin_left, margin_top,
                ),
            });
            ops.push(Op::PaintShading { id });
            ops.push(Op::RestoreGraphicsState);
        }

        DisplayListItem::PushOpacity { opacity, .. } => {
            // Approximate group opacity by setting fill+stroke alpha for the
            // wrapped content until PopOpacity. (True isolated group opacity
            // would need a transparency-group XObject; per-primitive alpha is
            // correct for non-overlapping content, which is the common case.)
            ops.push(Op::SaveGraphicsState);
            let a = (*opacity).clamp(0.0, 1.0);
            if a < 1.0 {
                let mut gs = ExtendedGraphicsState::default();
                gs.set_current_fill_alpha(a);
                gs.set_current_stroke_alpha(a);
                let id = bridge_res.add_extgstate(gs);
                ops.push(Op::LoadGraphicsState { gs: id });
            }
        }

        DisplayListItem::PopOpacity => {
            ops.push(Op::RestoreGraphicsState);
        }

        DisplayListItem::PushClip { bounds, border_radius } => {
            // Begin a clip scope: save the graphics state, then intersect the
            // current clip path with this (optionally rounded) rectangle. The
            // matching `PopClip` restores the state (un-clips). Nesting is handled
            // by PDF's own q/Q graphics-state stack, so no separate clip stack is
            // needed here. `PaintMode::Clip` makes `polygon_to_stream_ops` emit the
            // path followed by `W`/`W*` + `n` instead of a fill/stroke.
            ops.push(Op::SaveGraphicsState);

            let radii = crate::html::border::BorderRadii {
                top_left: (border_radius.top_left * CSS_PX_TO_PT, border_radius.top_left * CSS_PX_TO_PT),
                top_right: (border_radius.top_right * CSS_PX_TO_PT, border_radius.top_right * CSS_PX_TO_PT),
                bottom_right: (border_radius.bottom_right * CSS_PX_TO_PT, border_radius.bottom_right * CSS_PX_TO_PT),
                bottom_left: (border_radius.bottom_left * CSS_PX_TO_PT, border_radius.bottom_left * CSS_PX_TO_PT),
            };
            let has_radius = radii.top_left.0 > 0.0 || radii.top_right.0 > 0.0
                || radii.bottom_right.0 > 0.0 || radii.bottom_left.0 > 0.0;

            let polygon = if has_radius {
                let b = bounds_px_to_pt(bounds.inner());
                let points = crate::html::border::create_rounded_rect_path_with_margins(
                    b.origin.x, b.origin.y, b.size.width, b.size.height,
                    &radii, page_height, margin_left, margin_top,
                );
                crate::graphics::Polygon {
                    rings: vec![crate::graphics::PolygonRing { points }],
                    mode: crate::graphics::PaintMode::Clip,
                    winding_order: crate::graphics::WindingOrder::NonZero,
                }
            } else {
                let x = transform.x(bounds.origin().x);
                let y = transform.rect_y(bounds.origin().y, bounds.size().height);
                let w = transform.dim(bounds.size().width);
                let h = transform.dim(bounds.size().height);
                let mut p = make_rect_polygon_pt(x, y, w, h);
                p.mode = crate::graphics::PaintMode::Clip;
                p
            };
            ops.push(Op::DrawPolygon { polygon });
        }

        DisplayListItem::PopClip => {
            ops.push(Op::RestoreGraphicsState);
        }

        _ => {
            // Remaining items not yet wired for PDF:
            //  - PushImageMaskClip/PopImageMaskClip: alpha-mask clipping (would
            //    need an SMask); content currently renders unclipped (graceful).
            //  - PushScrollFrame/PushStackingContext/Push(Backdrop)Filter/
            //    PushTextShadow and their Pops, VirtualView*, ScrollBar*,
            //    Selection/Cursor (screen-only).
            // These intentionally emit nothing (and crucially no unbalanced q/Q).
        }
    }
}

/// Public API for rendering UnifiedLayout to PDF operations (without margins)
/// 
/// This is useful for rendering text layouts directly without going through
/// the full display list conversion.
pub fn render_unified_layout_public<T: ParsedFontTrait + 'static>(
    layout: &UnifiedLayout,
    bounds_width: f32,
    bounds_height: f32, 
    color: ColorU,
    page_height: f32,
    _font_manager: &FontManager<T>,
) -> Vec<Op> {
    let mut ops = Vec::new();
    let bounds = LogicalRect { 
        origin: LogicalPosition::new(0.0, 0.0),
        size: LogicalSize::new(bounds_width, bounds_height),
    };
    render_unified_layout_impl(&mut ops, layout, &bounds, color, page_height, _font_manager);
    ops
}

/// Implementation function for rendering UnifiedLayout to PDF operations
fn render_unified_layout_impl<T: ParsedFontTrait + 'static>(
    ops: &mut Vec<Op>,
    layout: &UnifiedLayout,
    bounds: &LogicalRect,
    _color: ColorU,  // Unused: per-glyph color from layout takes precedence
    page_height: f32,
    font_manager: &FontManager<T>,
) {
    use azul_layout::text3::glyphs::get_glyph_runs_pdf;

    // Get loaded fonts from font manager for glyph run extraction
    let loaded_fonts = font_manager.get_loaded_fonts();
    
    // Get PDF-optimized glyph runs (grouped by font/color/style/line)
    let glyph_runs = get_glyph_runs_pdf(layout, &loaded_fonts);

    if glyph_runs.is_empty() {
        return;
    }

    // Track current color to avoid redundant operations
    let mut current_color: Option<ColorU> = None;

    // Process each glyph run - each run will have its own text section
    for run in glyph_runs.iter() {
        if run.glyphs.is_empty() {
            continue;
        }

        // Set color if it changed (BEFORE text section)
        if current_color != Some(run.color) {
            ops.push(Op::SetFillColor {
                col: convert_color(&run.color),
            });
            current_color = Some(run.color);
        }

        // Set font if it changed OR if we're starting a new text section (BEFORE text section)
        // Note: Font must be set inside each text section (BT...ET), so we set it every time
        let font_id = FontId(format!("F{}", run.font_hash));
        ops.push(Op::SetFont {
            font: crate::ops::PdfFontHandle::External(font_id.clone()),
            size: Pt(run.font_size_px * CSS_PX_TO_PT),
        });

        // Start text section AFTER setting font and color
        ops.push(Op::StartTextSection);

        // Layout coordinates are in CSS px; convert to PDF pt.
        // Position each glyph absolutely using SetTextMatrix + ShowText
        // This gives us complete control over positioning for RTL, vertical text, etc.
        // and avoids relying on PDF's font metrics for cursor advancement
        for glyph in &run.glyphs {
            // Calculate absolute position for this glyph (in CSS px)
            let glyph_x_px = bounds.origin.x + glyph.position.x;
            let glyph_y_px = bounds.origin.y + glyph.position.y;
            
            // Convert from CSS px to PDF pt, then flip Y axis
            // HTML: origin at top-left, Y increases downward
            // PDF: origin at bottom-left, Y increases upward
            let pdf_x = glyph_x_px * CSS_PX_TO_PT;
            let pdf_y = page_height - glyph_y_px * CSS_PX_TO_PT;

            // Set the text matrix to position this specific glyph
            // The matrix values are in PDF user space units (points)
            ops.push(Op::SetTextMatrix {
                matrix: crate::matrix::TextMatrix::Raw([
                    1.0,    // a: Horizontal scaling (1.0 = no scaling)
                    0.0,    // b: Horizontal skewing
                    0.0,    // c: Vertical skewing
                    1.0,    // d: Vertical scaling (1.0 = no scaling)
                    pdf_x,  // e: Horizontal translation (in points)
                    pdf_y,  // f: Vertical translation (in points)
                ]),
            });

            // Render just this one glyph
            ops.push(Op::ShowText {
                items: vec![crate::text::TextItem::GlyphIds(vec![
                    crate::text::Codepoint {
                        gid: glyph.glyph_id,
                        offset: 0.0,
                        cid: Some(glyph.unicode_codepoint.clone()),
                    }
                ])],
            });
        }

        // End text section after this run
        ops.push(Op::EndTextSection);
    }
}

/// Render UnifiedLayout to PDF operations with margin support
fn render_unified_layout_with_margins<T: ParsedFontTrait + 'static>(
    ops: &mut Vec<Op>,
    layout: &UnifiedLayout,
    bounds: &LogicalRect,
    _color: ColorU,  // Unused: per-glyph color from layout takes precedence
    transform: &CoordTransform,
    font_manager: &FontManager<T>,
) {
    use azul_layout::text3::glyphs::get_glyph_runs_pdf;

    // Get loaded fonts from font manager for glyph run extraction
    let loaded_fonts = font_manager.get_loaded_fonts();
    
    // Get PDF-optimized glyph runs (grouped by font/color/style/line)
    let glyph_runs = get_glyph_runs_pdf(layout, &loaded_fonts);

    if glyph_runs.is_empty() {
        return;
    }

    // ========================================================================
    // FIRST PASS: Render all inline background colors BEFORE any text
    // ========================================================================
    // 
    // This two-pass approach ensures proper z-order:
    // - Pass 1: Draw all background rectangles (this loop)
    // - Pass 2: Draw all text on top of backgrounds (next loop)
    // 
    // Without this separation, backgrounds would be drawn interleaved with text,
    // potentially covering text from previous runs.
    // 
    // The background_color comes from CSS like: <span style="background-color: yellow">
    // It's propagated through: CSS -> StyleProperties -> ShapedGlyph -> PdfGlyphRun
    // 
    for run in glyph_runs.iter() {
        if run.glyphs.is_empty() {
            continue;
        }
        
        // Render background if present
        if let Some(bg_color) = run.background_color {
            if bg_color.a > 0 {
                // Calculate bounding box of this glyph run
                if let (Some(first_glyph), Some(last_glyph)) = 
                    (run.glyphs.first(), run.glyphs.last()) 
                {
                    let font_size = run.font_size_px;
                    // Estimate ascent/descent from font size (typical values)
                    let ascent = font_size * 0.8;
                    let descent = font_size * 0.2;
                    
                    // Calculate background rectangle in layout space
                    let bg_start_x = bounds.origin.x + first_glyph.position.x;
                    let bg_end_x = bounds.origin.x + last_glyph.position.x + last_glyph.advance;
                    let bg_width = bg_end_x - bg_start_x;
                    
                    // Background spans from ascent to descent relative to baseline
                    let baseline_y = bounds.origin.y + first_glyph.position.y;
                    let bg_top_y = baseline_y - ascent;
                    let bg_height = ascent + descent;
                    
                    // Transform to PDF coordinates (px → pt via transform)
                    let pdf_x = transform.x(bg_start_x);
                    let pdf_y = transform.rect_y(bg_top_y, bg_height);
                    
                    // Convert dimensions from CSS px to PDF pt
                    let pdf_w = transform.dim(bg_width);
                    let pdf_h = transform.dim(bg_height);
                    
                    ops.push(Op::SaveGraphicsState);
                    ops.push(Op::SetFillColor {
                        col: convert_color(&bg_color),
                    });
                    ops.push(Op::DrawPolygon { polygon: make_rect_polygon_pt(pdf_x, pdf_y, pdf_w, pdf_h) });
                    ops.push(Op::RestoreGraphicsState);
                }
            }
        }
    }

    // ========================================================================
    // SECOND PASS: Render all text AFTER backgrounds
    // ========================================================================
    //
    // Now that all backgrounds are drawn, render the text on top.
    // Each glyph run gets its own BT...ET text section for proper font handling.
    //
    let mut current_color: Option<ColorU> = None;

    for run in glyph_runs.iter() {
        if run.glyphs.is_empty() {
            continue;
        }

        // Set color if it changed (BEFORE text section)
        if current_color != Some(run.color) {
            ops.push(Op::SetFillColor {
                col: convert_color(&run.color),
            });
            current_color = Some(run.color);
        }

        // Set font (convert CSS px to PDF pt)
        let font_id = FontId(format!("F{}", run.font_hash));
        ops.push(Op::SetFont {
            font: crate::ops::PdfFontHandle::External(font_id.clone()),
            size: Pt(transform.dim(run.font_size_px)),
        });

        // Start text section AFTER setting font and color
        ops.push(Op::StartTextSection);

        // Position each glyph absolutely using SetTextMatrix + ShowText
        for glyph in &run.glyphs {
            // NOTE: Glyphs have already been filtered by the pagination/clipping code
            // in display_list.rs (clip_and_offset_display_item). The glyph positions
            // are page-relative and should be rendered directly.
            // 
            // The bounds.origin represents where this TextLayout block starts on the page,
            // and glyph.position is relative to the block origin (after the clipping pass).
            
            // Calculate absolute position for this glyph in layout space
            let glyph_x_layout = bounds.origin.x + glyph.position.x;
            let glyph_y_layout = bounds.origin.y + glyph.position.y;
            
            // Transform to PDF coordinate system with margins:
            // - Add margin_left to X
            // - Flip Y and subtract margin_top
            let pdf_x = transform.x(glyph_x_layout);
            let pdf_y = transform.y(glyph_y_layout);
            
            // Set the text matrix to position this specific glyph
            ops.push(Op::SetTextMatrix {
                matrix: crate::matrix::TextMatrix::Raw([
                    1.0,    // a: Horizontal scaling
                    0.0,    // b: Horizontal skewing
                    0.0,    // c: Vertical skewing
                    1.0,    // d: Vertical scaling
                    pdf_x,  // e: Horizontal translation (in points)
                    pdf_y,  // f: Vertical translation (in points)
                ]),
            });

            // Render just this one glyph
            ops.push(Op::ShowText {
                items: vec![crate::text::TextItem::GlyphIds(vec![
                    crate::text::Codepoint {
                        gid: glyph.glyph_id,
                        offset: 0.0,
                        cid: Some(glyph.unicode_codepoint.clone()),
                    }
                ])],
            });
        }

        // End text section after this run
        ops.push(Op::EndTextSection);
    }
    
}

/// Apply margin offset to all PDF operations.
/// 
/// This shifts all coordinates by the specified margins:
/// - `offset_x` shifts content to the right (for left margin)
/// - `offset_y` shifts content up (for bottom margin in PDF coordinate system)
pub fn apply_margin_offset(ops: &mut [Op], offset_x: crate::Mm, offset_y: crate::Mm) {
    // Skip if no offset needed
    if offset_x.0 == 0.0 && offset_y.0 == 0.0 {
        return;
    }
    
    // Convert mm offsets to pt (1 mm = 2.83465 pt)
    let offset_x_pt = crate::Pt(offset_x.0 * 2.83465);
    let offset_y_pt = crate::Pt(offset_y.0 * 2.83465);
    
    for op in ops.iter_mut() {
        match op {
            Op::DrawPolygon { polygon } => {
                for ring in &mut polygon.rings {
                    for point in &mut ring.points {
                        point.p.x = crate::Pt(point.p.x.0 + offset_x_pt.0);
                        point.p.y = crate::Pt(point.p.y.0 + offset_y_pt.0);
                    }
                }
            }
            Op::DrawLine { line } => {
                for point in &mut line.points {
                    point.p.x = crate::Pt(point.p.x.0 + offset_x_pt.0);
                    point.p.y = crate::Pt(point.p.y.0 + offset_y_pt.0);
                }
            }
            Op::SetTextCursor { pos } => {
                pos.x = crate::Pt(pos.x.0 + offset_x_pt.0);
                pos.y = crate::Pt(pos.y.0 + offset_y_pt.0);
            }
            Op::UseXobject { transform, .. } => {
                // Adjust the translation in the transform
                transform.translate_x = Some(crate::Pt(
                    transform.translate_x.unwrap_or(crate::Pt(0.0)).0 + offset_x_pt.0
                ));
                transform.translate_y = Some(crate::Pt(
                    transform.translate_y.unwrap_or(crate::Pt(0.0)).0 + offset_y_pt.0
                ));
            }
            Op::DrawRectangle { rectangle } => {
                rectangle.x = crate::Pt(rectangle.x.0 + offset_x_pt.0);
                rectangle.y = crate::Pt(rectangle.y.0 + offset_y_pt.0);
            }
            Op::LinkAnnotation { link } => {
                link.rect.x = crate::Pt(link.rect.x.0 + offset_x_pt.0);
                link.rect.y = crate::Pt(link.rect.y.0 + offset_y_pt.0);
            }
            // These operations don't have coordinates that need adjustment
            Op::Marker { .. }
            | Op::SetColorSpaceStroke { .. }
            | Op::SetColorSpaceFill { .. }
            | Op::BeginLayer { .. }
            | Op::EndLayer
            | Op::SaveGraphicsState
            | Op::RestoreGraphicsState
            | Op::LoadGraphicsState { .. }
            | Op::PaintShading { .. }
            | Op::StartTextSection
            | Op::EndTextSection
            | Op::SetFont { .. }
            | Op::ShowText { .. }
            | Op::AddLineBreak
            | Op::SetLineHeight { .. }
            | Op::SetWordSpacing { .. }
            | Op::SetFillColor { .. }
            | Op::SetOutlineColor { .. }
            | Op::SetOutlineThickness { .. }
            | Op::SetLineDashPattern { .. }
            | Op::SetLineJoinStyle { .. }
            | Op::SetLineCapStyle { .. }
            | Op::SetMiterLimit { .. }
            | Op::SetTextRenderingMode { .. }
            | Op::SetCharacterSpacing { .. }
            | Op::SetLineOffset { .. }
            | Op::SetTransformationMatrix { .. }
            | Op::SetTextMatrix { .. }
            | Op::MoveTextCursorAndSetLeading { .. }
            | Op::SetRenderingIntent { .. }
            | Op::SetHorizontalScaling { .. }
            | Op::BeginInlineImage
            | Op::BeginInlineImageData
            | Op::EndInlineImage
            | Op::BeginMarkedContent { .. }
            | Op::BeginMarkedContentWithProperties { .. }
            | Op::BeginOptionalContent { .. }
            | Op::DefineMarkedContentPoint { .. }
            | Op::EndMarkedContent
            | Op::EndMarkedContentWithProperties
            | Op::EndOptionalContent
            | Op::BeginCompatibilitySection
            | Op::EndCompatibilitySection
            | Op::MoveToNextLineShowText { .. }
            | Op::SetSpacingMoveAndShowText { .. }
            | Op::Unknown { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use azul_core::geom::{LogicalPosition, LogicalRect, LogicalSize};
    use azul_core::resources::{ImageRef, RawImageFormat};
    use azul_layout::solver3::display_list::{
        BorderRadius, DisplayList, DisplayListItem, WindowLogicalRect,
    };
    use azul_layout::text3::cache::FontManager;

    fn empty_font_manager() -> FontManager<azul_css::props::basic::FontRef> {
        FontManager::new(rust_fontconfig::FcFontCache::default())
            .expect("build empty FontManager")
    }

    /// The real cat.jpg used by the `html_image` example, decoded and placed in a
    /// display-list Image item, must produce an `Op::UseXobject` referencing the
    /// deterministic id and scaled to the item bounds.
    #[test]
    fn image_item_emits_use_xobject() {
        let cat_jpg: &[u8] = include_bytes!("../../examples/assets/img/cat.jpg");
        let resolved = resolve_html_images(
            &[("cat.jpg".to_string(), cat_jpg.to_vec())]
                .into_iter()
                .collect(),
        );
        let (_id, raw) = resolved.get("cat.jpg").expect("cat.jpg decoded");
        assert!(raw.width > 0 && raw.height > 0, "decoded image has dimensions");

        // ImageRef carrying the src as its NullImage tag (mirrors what azul's
        // xml_node_to_dom_fast produces for <img src="cat.jpg">).
        let image_ref = ImageRef::null_image(
            raw.width,
            raw.height,
            RawImageFormat::RGBA8,
            b"cat.jpg".to_vec(),
        );

        // 200x100 px box at (10, 20) px.
        let bounds = WindowLogicalRect(LogicalRect {
            origin: LogicalPosition { x: 10.0, y: 20.0 },
            size: LogicalSize { width: 200.0, height: 100.0 },
        });

        let mut dl = DisplayList::default();
        dl.items.push(DisplayListItem::Image {
            bounds,
            image: image_ref,
            border_radius: BorderRadius::default(),
        });

        let fm = empty_font_manager();
        let page = LogicalSize { width: 595.0, height: 842.0 }; // A4 pt
        let ops = display_list_to_printpdf_ops_with_margins(&dl, page, 0.0, 0.0, &fm, &resolved, &mut BridgeResources::default())
            .expect("bridge conversion");

        let use_xobject = ops.iter().find_map(|op| match op {
            Op::UseXobject { id, transform } => Some((id.clone(), *transform)),
            _ => None,
        });
        let (id, transform) = use_xobject.expect("an Op::UseXobject must be emitted for the image");

        assert_eq!(id.0, "HtmlImg_cat_jpg", "deterministic xobject id");
        assert_eq!(id, image_xobject_id("cat.jpg"), "id matches helper");

        // The image should be scaled to fill the 200x100 px box (= 150x75 pt).
        const IMAGE_DPI: f32 = 300.0;
        let natural_w_pt = crate::Px(raw.width).into_pt(IMAGE_DPI).0;
        let natural_h_pt = crate::Px(raw.height).into_pt(IMAGE_DPI).0;
        let target_w_pt = 200.0 * (72.0 / 96.0);
        let target_h_pt = 100.0 * (72.0 / 96.0);
        let sx = transform.scale_x.expect("scale_x set");
        let sy = transform.scale_y.expect("scale_y set");
        assert!((natural_w_pt * sx - target_w_pt).abs() < 0.5, "image width fills bounds");
        assert!((natural_h_pt * sy - target_h_pt).abs() < 0.5, "image height fills bounds");

        // Bottom-left corner: x = 10px -> pt + left margin(0); y flipped.
        let tx = transform.translate_x.expect("translate_x set").0;
        let ty = transform.translate_y.expect("translate_y set").0;
        assert!((tx - 10.0 * (72.0 / 96.0)).abs() < 0.5, "x placement");
        // page_height - (y + h)*px_to_pt = 842 - (20+100)*0.75
        let expect_ty = 842.0 - (20.0 + 100.0) * (72.0 / 96.0);
        assert!((ty - expect_ty).abs() < 0.5, "y placement (bottom-left, flipped)");
    }

    /// An image whose src has no matching entry in the resolved map renders
    /// nothing (no UseXobject, no panic).
    #[test]
    fn image_item_without_bytes_is_noop() {
        let image_ref =
            ImageRef::null_image(100, 50, RawImageFormat::RGBA8, b"missing.png".to_vec());
        let bounds = WindowLogicalRect(LogicalRect {
            origin: LogicalPosition { x: 0.0, y: 0.0 },
            size: LogicalSize { width: 100.0, height: 50.0 },
        });
        let mut dl = DisplayList::default();
        dl.items.push(DisplayListItem::Image {
            bounds,
            image: image_ref,
            border_radius: BorderRadius::default(),
        });

        let fm = empty_font_manager();
        let ops = display_list_to_printpdf_ops_with_margins(
            &dl,
            LogicalSize { width: 595.0, height: 842.0 },
            0.0,
            0.0,
            &fm,
            &ResolvedImages::new(),
            &mut BridgeResources::default(),
        )
        .expect("bridge conversion");

        assert!(
            !ops.iter().any(|op| matches!(op, Op::UseXobject { .. })),
            "no UseXobject when bytes are missing"
        );
    }

    /// A translucent Rect (alpha < 255) must emit a LoadGraphicsState op and
    /// record exactly one ExtGState carrying the fill alpha on BridgeResources.
    #[test]
    fn translucent_rect_emits_extgstate_alpha() {
        let bounds = WindowLogicalRect(LogicalRect {
            origin: LogicalPosition { x: 0.0, y: 0.0 },
            size: LogicalSize { width: 50.0, height: 50.0 },
        });
        let mut dl = DisplayList::default();
        dl.items.push(DisplayListItem::Rect {
            bounds,
            color: ColorU { r: 255, g: 0, b: 0, a: 128 }, // 50% red
            border_radius: BorderRadius::default(),
        });

        let fm = empty_font_manager();
        let mut bridge_res = BridgeResources::default();
        let ops = display_list_to_printpdf_ops_with_margins(
            &dl,
            LogicalSize { width: 595.0, height: 842.0 },
            0.0,
            0.0,
            &fm,
            &ResolvedImages::new(),
            &mut bridge_res,
        )
        .expect("bridge conversion");

        assert!(
            ops.iter().any(|op| matches!(op, Op::LoadGraphicsState { .. })),
            "translucent fill must load an ExtGState"
        );
        assert_eq!(bridge_res.extgstates.len(), 1, "one ExtGState recorded");
        let (_, gs) = &bridge_res.extgstates[0];
        assert!((gs.current_fill_alpha() - 128.0 / 255.0).abs() < 1e-3, "fill alpha set");
    }

    /// An opaque Rect (alpha == 255) must NOT create any ExtGState.
    #[test]
    fn opaque_rect_has_no_extgstate() {
        let bounds = WindowLogicalRect(LogicalRect {
            origin: LogicalPosition { x: 0.0, y: 0.0 },
            size: LogicalSize { width: 50.0, height: 50.0 },
        });
        let mut dl = DisplayList::default();
        dl.items.push(DisplayListItem::Rect {
            bounds,
            color: ColorU { r: 0, g: 0, b: 255, a: 255 },
            border_radius: BorderRadius::default(),
        });
        let fm = empty_font_manager();
        let mut bridge_res = BridgeResources::default();
        let _ = display_list_to_printpdf_ops_with_margins(
            &dl, LogicalSize { width: 595.0, height: 842.0 }, 0.0, 0.0, &fm,
            &ResolvedImages::new(), &mut bridge_res,
        ).expect("bridge conversion");
        assert!(bridge_res.extgstates.is_empty(), "opaque fill needs no ExtGState");
    }

    /// End-to-end serialization: a document carrying the decoded image as an
    /// Image XObject must serialize to a PDF containing `/Subtype /Image`.
    #[test]
    fn registered_image_serializes_as_xobject() {
        let cat_jpg: &[u8] = include_bytes!("../../examples/assets/img/cat.jpg");
        let mut warnings = Vec::new();
        let raw = RawImage::decode_from_bytes(cat_jpg, &mut warnings).expect("decode cat.jpg");

        let mut doc = crate::PdfDocument::new("img test");
        let id = image_xobject_id("cat.jpg");
        doc.resources
            .xobjects
            .map
            .insert(id.clone(), crate::XObject::Image(raw.clone()));
        // A page that uses the image so the XObject is referenced.
        doc.pages.push(crate::PdfPage::new(
            crate::Mm(210.0),
            crate::Mm(297.0),
            vec![Op::UseXobject {
                id,
                transform: XObjectTransform::default(),
            }],
        ));

        let bytes = doc.save(&crate::PdfSaveOptions::default(), &mut Vec::new());
        // lopdf renders the dict without spaces (e.g. `/Subtype/Image`); accept
        // both forms to be robust to lopdf formatting changes.
        let contains = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        let has_image = contains(b"/Subtype/Image") || contains(b"/Subtype /Image");
        assert!(
            has_image,
            "serialized PDF must contain an Image XObject (len={})",
            bytes.len()
        );
        // The cat.jpg is a JPEG, so it round-trips as a DCTDecode-filtered stream.
        // (The serializer may down-scale the pixels to fit a size budget, so we do
        // not assert exact dimensions — only that an image stream is present.)
        let has_filter = contains(b"/Filter/DCTDecode") || contains(b"/Filter /DCTDecode");
        assert!(has_filter, "image XObject should be a DCTDecode (JPEG) stream");
        let has_width = contains(b"/Width") && contains(b"/Height");
        assert!(has_width, "serialized PDF must declare image width/height");
    }
}
