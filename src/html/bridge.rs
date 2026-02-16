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

use azul_core::{
    geom::{LogicalRect, LogicalSize, LogicalPosition},
};
use azul_css::props::basic::ColorU;
use azul_layout::{
    solver3::display_list::{DisplayList, DisplayListItem},
    text3::cache::{FontManager, ParsedFontTrait, UnifiedLayout},
};

use crate::{Color, Mm, Op, Pt, Rgb, FontId};

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
            font_manager
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
    display_list_to_printpdf_ops_with_margins(display_list, page_size, 0.0, 0.0, font_manager)
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
) {
    let transform = CoordTransform::new(page_height, margin_left, margin_top);
    
    match item {
        DisplayListItem::Rect {
            bounds,
            color,
            border_radius,
        } => {
            // Skip rectangles with zero size (layout artifacts from empty inline elements)
            if bounds.size.width == 0.0 || bounds.size.height == 0.0 {
                return;
            }
            
            // Convert rectangle to PDF polygon
            ops.push(Op::SaveGraphicsState);

            // Convert DisplayList BorderRadius to border module BorderRadii
            let radii = crate::html::border::BorderRadii {
                top_left: (border_radius.top_left, border_radius.top_left),
                top_right: (border_radius.top_right, border_radius.top_right),
                bottom_right: (border_radius.bottom_right, border_radius.bottom_right),
                bottom_left: (border_radius.bottom_left, border_radius.bottom_left),
            };
            
            // Check if we have border radius
            let has_radius = radii.top_left.0 > 0.0 || radii.top_left.1 > 0.0
                || radii.top_right.0 > 0.0 || radii.top_right.1 > 0.0
                || radii.bottom_right.0 > 0.0 || radii.bottom_right.1 > 0.0
                || radii.bottom_left.0 > 0.0 || radii.bottom_left.1 > 0.0;
            
            if has_radius {
                // Use rounded rectangle path for filling (with margin-adjusted coordinates)
                // Convert layout coordinates from CSS px to PDF pt before passing
                let b = bounds_px_to_pt(bounds);
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
                let x = transform.x(bounds.origin.x);
                let y = transform.rect_y(bounds.origin.y, bounds.size.height);
                let w = transform.dim(bounds.size.width);
                let h = transform.dim(bounds.size.height);

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
                render_unified_layout_with_margins(ops, unified_layout, bounds, *color, &transform, font_manager);
                
                // Also update the current text layout for any subsequent processing
                *current_text_layout = Some((unified_layout, *bounds));
            }
        }

        DisplayListItem::Text { glyphs, font_hash, font_size_px, color, clip_rect: _ } => {
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
            let bounds_pt = bounds_px_to_pt(bounds);
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

        DisplayListItem::Image { bounds: _, image: _ } => {
            // Image rendering - not yet implemented
        }

        _ => {
            // Other display list items not yet implemented
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

        // TODO: Handle text decorations (underline, strikethrough, overline)
        // This would require drawing lines at appropriate positions relative to the baseline
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
