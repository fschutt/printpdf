//! Bridge module to translate azul-layout PDF operations to printpdf operations.
//!
//! This module converts the intermediate PDF representation from azul-layout
//! into printpdf's native Op enum, allowing us to leverage azul's layout engine
//! while using printpdf's PDF generation.
//!
//! IMPORTANT: This module now converts DisplayList directly to printpdf Ops,
//! bypassing the intermediate azul PdfOp representation. This allows us to
//! generate WriteCodepoints (glyph IDs) instead of WriteText (text strings),
//! which is necessary for proper text shaping in complex scripts.

use azul_core::{
    geom::LogicalSize,
    ui_solver::GlyphInstance,
};
use azul_css::props::basic::ColorU;
use azul_layout::{
    solver3::display_list::{DisplayList, DisplayListItem},
    text3::cache::FontHash,
};
use std::collections::BTreeMap;

use crate::{Color, Mm, Op, Pt, Rgb, FontId};

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
/// WriteCodepoints (glyph IDs) instead of WriteText (text strings).
pub fn display_list_to_printpdf_ops(
    display_list: &DisplayList,
    page_size: LogicalSize,
    font_id_map: &mut BTreeMap<FontHash, FontId>,
) -> Vec<Op> {
    let mut ops = Vec::new();
    let page_height = page_size.height;

    println!("[bridge] Converting DisplayList with {} items", display_list.items.len());

    for (idx, item) in display_list.items.iter().enumerate() {
        println!("[bridge] Item {}: {:?}", idx, match item {
            DisplayListItem::Text { .. } => "Text",
            DisplayListItem::Rect { .. } => "Rect",
            DisplayListItem::Border { .. } => "Border",
            DisplayListItem::Image { .. } => "Image",
            _ => "Other",
        });
        convert_display_list_item(item, &mut ops, font_id_map, page_height);
    }

    println!("[bridge] Generated {} ops", ops.len());
    ops
}

fn convert_display_list_item(
    item: &DisplayListItem,
    ops: &mut Vec<Op>,
    font_id_map: &mut BTreeMap<FontHash, FontId>,
    page_height: f32,
) {
    match item {
        DisplayListItem::Rect {
            bounds,
            color,
            border_radius: _,
        } => {
            // Convert rectangle to PDF polygon
            ops.push(Op::SaveGraphicsState);

            let y = page_height - bounds.origin.y - bounds.size.height;

            // Simple rectangle (ignore border_radius for now)
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
            ops.push(Op::RestoreGraphicsState);
        }

        DisplayListItem::Text {
            glyphs,
            font_hash,
            font_size_px,
            color,
            clip_rect: _,
        } => {
            // Skip empty text
            if glyphs.is_empty() {
                println!("[bridge] Skipping empty text");
                return;
            }

            println!("[bridge] Converting text with {} glyphs, font_hash={:?}, font_size={}",
                glyphs.len(), font_hash, font_size_px);

            // Get or create printpdf font ID for this font
            let font_id = font_id_map
                .entry(*font_hash)
                .or_insert_with(|| {
                    let id = FontId(format!("F{}", font_hash.font_hash));
                    println!("[bridge] Created new font ID: {:?} for font_hash {:?}", id, font_hash);
                    id
                })
                .clone();

            ops.push(Op::StartTextSection);
            ops.push(Op::SetFillColor {
                col: convert_color(color),
            });
            ops.push(Op::SetFontSize {
                size: Pt(*font_size_px),
                font: font_id.clone(),
            });

            // Group glyphs by line (same Y position within tolerance)
            let mut current_line_glyphs: Vec<&GlyphInstance> = Vec::new();
            let mut current_y = None;

            for glyph in glyphs {
                let glyph_y = page_height - glyph.point.y;
                
                // Check if we're still on the same line (within 0.5pt tolerance)
                let same_line = current_y.map_or(true, |y: f32| (y - glyph_y).abs() < 0.5);
                
                if same_line {
                    current_line_glyphs.push(glyph);
                    current_y = Some(glyph_y);
                } else {
                    // Render the previous line
                    if !current_line_glyphs.is_empty() {
                        render_glyph_line(ops, &current_line_glyphs, &font_id, current_y.unwrap());
                        current_line_glyphs.clear();
                    }
                    // Start new line
                    current_line_glyphs.push(glyph);
                    current_y = Some(glyph_y);
                }
            }

            // Render remaining glyphs
            if !current_line_glyphs.is_empty() {
                println!("[bridge] Rendering final line with {} glyphs", current_line_glyphs.len());
                render_glyph_line(ops, &current_line_glyphs, &font_id, current_y.unwrap());
            }

            ops.push(Op::EndTextSection);
        }

        DisplayListItem::Border {
            bounds,
            widths,
            colors,
            styles: _,
            border_radius: _,
        } => {
            // Simplified border rendering

            let width = widths
                .top
                .and_then(|w| w.get_property().cloned())
                .map(|w| w.inner.to_pixels(0.0))
                .unwrap_or(0.0);

            if width > 0.0 {
                let color = colors
                    .top
                    .and_then(|c| c.get_property().cloned())
                    .map(|c| c.inner)
                    .unwrap_or(ColorU {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    });

                ops.push(Op::SaveGraphicsState);

                let y = page_height - bounds.origin.y - bounds.size.height;

                let line = crate::graphics::Line {
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
                    is_closed: true,
                };

                ops.push(Op::SetOutlineColor {
                    col: convert_color(&color),
                });
                ops.push(Op::SetOutlineThickness { pt: Pt(width) });
                ops.push(Op::DrawLine { line });

                ops.push(Op::RestoreGraphicsState);
            }
        }

        DisplayListItem::Image { bounds: _, key: _ } => {
            // Image rendering - not yet implemented
            println!("[bridge] Image rendering not yet implemented");
        }

        _ => {
            // Other display list items not yet implemented
            println!("[bridge] Unsupported display list item");
        }
    }
}

/// Helper function to render a line of glyphs using WriteCodepoints
fn render_glyph_line(
    ops: &mut Vec<Op>,
    glyphs: &[&GlyphInstance],
    font_id: &FontId,
    y_pos: f32,
) {
    if glyphs.is_empty() {
        return;
    }

    println!("[bridge] Rendering glyph line with {} glyphs at y={}", glyphs.len(), y_pos);

    // Set text position to the start of the line
    let first_x = glyphs[0].point.x;
    ops.push(Op::SetTextCursor {
        pos: crate::graphics::Point::new(Mm(first_x * 0.3527777778), Mm(y_pos * 0.3527777778)),
    });

    // Build glyph array with positioning
    // We need to generate (glyph_id, unicode_char) pairs for WriteCodepoints
    let mut codepoints: Vec<(u16, char)> = Vec::new();
    
    for glyph in glyphs {
        // GlyphInstance.index is the glyph ID (u32)
        // We need to convert it to u16 for WriteCodepoints
        let glyph_id = (glyph.index & 0xFFFF) as u16;
        
        // Use a placeholder character - in a real implementation, we'd need
        // to look up the Unicode codepoint from the font's cmap table
        // For now, use the replacement character
        let unicode_char = '\u{FFFD}'; // Replacement character
        
        codepoints.push((glyph_id, unicode_char));
    }

    println!("[bridge] Generated {} codepoints", codepoints.len());

    if !codepoints.is_empty() {
        ops.push(Op::WriteCodepoints {
            font: font_id.clone(),
            cp: codepoints,
        });
    }
}
