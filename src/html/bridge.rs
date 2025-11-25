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
use azul_css::props::basic::{pixel::DEFAULT_FONT_SIZE, ColorU};
use azul_layout::{
    solver3::display_list::{DisplayList, DisplayListItem},
    text3::cache::{FontLoaderTrait, FontManager, ParsedFontTrait, UnifiedLayout},
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

/// Convert a display list directly to printpdf Ops.
/// This bypasses the intermediate azul PdfOp format to generate
/// WriteCodepoints has been replaced with ShowText (use SetFont first to set the font).
pub fn display_list_to_printpdf_ops<T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    display_list: &DisplayList,
    page_size: LogicalSize,
    font_manager: &FontManager<T, Q>,
) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    let page_height = page_size.height;
    
    // Track the current TextLayout for glyph-to-unicode mapping
    let mut current_text_layout: Option<(&azul_layout::text3::cache::UnifiedLayout<T>, LogicalRect)> = None;

    for (idx, item) in display_list.items.iter().enumerate() {
        let item_type = match item {
            DisplayListItem::TextLayout { .. } => "TextLayout",
            DisplayListItem::Text { .. } => "Text",
            DisplayListItem::Rect { .. } => "Rect",
            DisplayListItem::Border { .. } => "Border",
            DisplayListItem::Image { .. } => "Image",
            DisplayListItem::SelectionRect { .. } => "SelectionRect",
            DisplayListItem::CursorRect { .. } => "CursorRect",
            DisplayListItem::Underline { .. } => "Underline",
            DisplayListItem::Strikethrough { .. } => "Strikethrough",
            DisplayListItem::Overline { .. } => "Overline",
            DisplayListItem::ScrollBar { .. } => "ScrollBar",
            DisplayListItem::IFrame { .. } => "IFrame",
            DisplayListItem::PushStackingContext { .. } => "PushStackingContext",
            DisplayListItem::PopStackingContext => "PopStackingContext",
            DisplayListItem::PushClip { .. } => "PushClip",
            DisplayListItem::PopClip => "PopClip",
            DisplayListItem::PushScrollFrame { .. } => "PushScrollFrame",
            DisplayListItem::PopScrollFrame => "PopScrollFrame",
            DisplayListItem::HitTestArea { .. } => "HitTestArea",
        };
        convert_display_list_item(&mut ops, item, page_height, &mut current_text_layout, font_manager);
    }

    // Process any remaining TextLayout that was collected (should be rare now)
    if let Some((layout, bounds)) = current_text_layout {
        // Note: This will likely be redundant since TextLayouts are processed immediately now
    }

    Ok(ops)
}

fn convert_display_list_item<'a, T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    ops: &mut Vec<Op>,
    item: &'a DisplayListItem,
    page_height: f32,
    current_text_layout: &mut Option<(&'a UnifiedLayout<T>, LogicalRect)>,
    font_manager: &FontManager<T, Q>,
) {
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
                // Use rounded rectangle path for filling
                let points = crate::html::border::create_rounded_rect_path_public(
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                    &radii,
                    page_height,
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
                let y = page_height - bounds.origin.y - bounds.size.height;
                
                let polygon = crate::graphics::Polygon {
                    rings: vec![crate::graphics::PolygonRing {
                        points: vec![
                            crate::graphics::LinePoint {
                                p: crate::graphics::Point::new(
                                    Mm(bounds.origin.x * 0.3527777778),
                                    Mm(y * 0.3527777778),
                                ),
                                bezier: false,
                            },
                            crate::graphics::LinePoint {
                                p: crate::graphics::Point::new(
                                    Mm((bounds.origin.x + bounds.size.width) * 0.3527777778),
                                    Mm(y * 0.3527777778),
                                ),
                                bezier: false,
                            },
                            crate::graphics::LinePoint {
                                p: crate::graphics::Point::new(
                                    Mm((bounds.origin.x + bounds.size.width) * 0.3527777778),
                                    Mm((y + bounds.size.height) * 0.3527777778),
                                ),
                                bezier: false,
                            },
                            crate::graphics::LinePoint {
                                p: crate::graphics::Point::new(
                                    Mm(bounds.origin.x * 0.3527777778),
                                    Mm((y + bounds.size.height) * 0.3527777778),
                                ),
                                bezier: false,
                            },
                        ],
                    }],
                    mode: crate::graphics::PaintMode::Fill,
                    winding_order: crate::graphics::WindingOrder::NonZero,
                };

                ops.push(Op::SetFillColor {
                    col: convert_color(color),
                });
                ops.push(Op::DrawPolygon { polygon });
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
            if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout<T>>() {
                // Process this TextLayout immediately instead of just storing it
                render_unified_layout_impl(ops, unified_layout, bounds, *color, page_height, font_manager);
                
                // Also update the current text layout for any subsequent processing
                *current_text_layout = Some((unified_layout, *bounds));
            }
        }

        DisplayListItem::Text { glyphs, .. } => {
            // IGNORE: Text items are for visual renderers, not PDF generation
            // The azul-layout code pushes TextLayout items BEFORE Text items
            // We only process the TextLayout items which contain the full UnifiedLayout
        }

        DisplayListItem::Border {
            bounds,
            widths,
            colors,
            styles,
            border_radius,
        } => {
            // Use comprehensive border rendering with full support for:
            // - All four sides rendered individually
            // - Border-radius with proper corner curves
            // - Border styles (solid, dashed, dotted, double, etc.)
            
            let config = BorderConfig {
                bounds: *bounds,
                widths: extract_border_widths(widths),
                colors: extract_border_colors(colors),
                styles: extract_border_styles(styles),
                radii: extract_border_radii(border_radius),
                page_height,
            };
            
            render_border(ops, &config);
        }

        DisplayListItem::Image { bounds: _, key: _ } => {
            // Image rendering - not yet implemented
        }

        _ => {
            // Other display list items not yet implemented
        }
    }
}

/// Render an entire UnifiedLayout to PDF operations
fn render_unified_layout<T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    ops: &mut Vec<Op>,
    layout: &UnifiedLayout<T>,
    bounds: &LogicalRect,
    color: ColorU,
    page_height: f32,
    _font_manager: &FontManager<T, Q>,
) {
    render_unified_layout_impl(ops, layout, bounds, color, page_height, _font_manager);
}

/// Public API for rendering UnifiedLayout to PDF operations
pub fn render_unified_layout_public<T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    layout: &UnifiedLayout<T>,
    bounds_width: f32,
    bounds_height: f32, 
    color: ColorU,
    page_height: f32,
    _font_manager: &FontManager<T, Q>,
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
fn render_unified_layout_impl<T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    ops: &mut Vec<Op>,
    layout: &UnifiedLayout<T>,
    bounds: &LogicalRect,
    color: ColorU,
    page_height: f32,
    _font_manager: &FontManager<T, Q>,
) {
    use azul_layout::text3::glyphs::get_glyph_runs_pdf;

    // Get PDF-optimized glyph runs (grouped by font/color/style/line)
    let glyph_runs = get_glyph_runs_pdf(layout);

    if glyph_runs.is_empty() {
        return;
    }

    // Track current state to avoid redundant operations
    let mut current_font_hash: Option<u64> = None;
    let mut current_font_size: Option<f32> = None;
    let mut current_color: Option<ColorU> = None;

    // Process each glyph run - each run will have its own text section
    for (run_idx, run) in glyph_runs.iter().enumerate() {
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
            size: Pt(run.font_size_px),
        });
        current_font_hash = Some(run.font_hash);
        current_font_size = Some(run.font_size_px);

        // Start text section AFTER setting font and color
        ops.push(Op::StartTextSection);

        // IMPORTANT: Unit conversion notes
        // 
        // The azul-layout DisplayList uses LogicalSize with coordinates in CSS pixels at 72 DPI.
        // This is the web standard where 1 CSS pixel = 1/96 inch at 96 DPI reference resolution,
        // BUT azul normalizes to 72 DPI (1 CSS px = 1 PDF pt).
        //
        // Page dimensions from GeneratePdfOptions are in millimeters (Mm), which get converted
        // to points when creating the LogicalSize: 210mm × (72/25.4) = 595.28pt
        //
        // Therefore:
        // - page_height: f32 = 595.28 (in points, from LogicalSize.height)
        // - bounds.origin.{x,y}: f32 = layout coordinates (in points)
        // - glyph.position.{x,y}: f32 = glyph coordinates relative to bounds (in points)
        //
        // All coordinates are ALREADY in PDF points (72 DPI), so no conversion is needed.
        // The TextMatrix values should be passed directly as raw floats in point units.

        // Position each glyph absolutely using SetTextMatrix + ShowText
        // This gives us complete control over positioning for RTL, vertical text, etc.
        // and avoids relying on PDF's font metrics for cursor advancement
        for glyph in &run.glyphs {
            // Calculate absolute position for this glyph
            // All values are in points (72 DPI) - no conversion needed
            let glyph_x_pt = bounds.origin.x + glyph.position.x;
            let glyph_y_pt = bounds.origin.y + glyph.position.y;
            
            // Convert from HTML coordinate system to PDF coordinate system
            // HTML: origin at top-left, Y increases downward
            // PDF: origin at bottom-left, Y increases upward
            // Therefore: PDF_Y = page_height - HTML_Y
            let pdf_x = glyph_x_pt;
            let pdf_y = page_height - glyph_y_pt;

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
