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
    geom::{LogicalRect, LogicalSize},
    ui_solver::GlyphInstance,
};
use azul_css::props::basic::{pixel::DEFAULT_FONT_SIZE, ColorU};
use azul_layout::{
    solver3::display_list::{DisplayList, DisplayListItem},
    text3::cache::{FontLoaderTrait, FontManager, ParsedFontTrait, UnifiedLayout},
};

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
pub fn display_list_to_printpdf_ops<T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    display_list: &DisplayList,
    page_size: LogicalSize,
    font_manager: &FontManager<T, Q>,
) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    let page_height = page_size.height;
    
    // Track the current TextLayout for glyph-to-unicode mapping
    let mut current_text_layout: Option<(&azul_layout::text3::cache::UnifiedLayout<T>, LogicalRect)> = None;

    println!("[bridge] Converting DisplayList with {} items", display_list.items.len());

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
        println!("[bridge] Item {}: {}", idx, item_type);
        convert_display_list_item(&mut ops, item, page_height, &mut current_text_layout, font_manager);
    }

    println!("[bridge] Generated {} ops", ops.len());
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

        DisplayListItem::TextLayout {
            layout,
            bounds,
            font_hash: _,
            font_size_px: _,
            color: _,
        } => {
            // Extract the UnifiedLayout from the type-erased Arc<dyn Any>
            if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout<T>>() {
                println!("[bridge] ✓ Found TextLayout with {} items, bounds={:?}", unified_layout.items.len(), bounds);
                *current_text_layout = Some((unified_layout, *bounds));
            } else {
                println!("[bridge] ✗ Failed to downcast TextLayout");
            }
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

            // DEBUG: Print first few glyph positions
            for (i, g) in glyphs.iter().enumerate().take(5) {
                println!("[bridge]   Glyph {}: index={}, point=({}, {})", 
                    i, g.index, g.point.x, g.point.y);
            }

            // Create printpdf font ID directly from font_hash
            let font_id = FontId(format!("F{}", font_hash.font_hash));
            println!("[bridge] Using font ID: {:?} for font_hash {:?}", font_id, font_hash);

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
                // glyph.point.y is relative to the container, keep it that way
                let glyph_y = glyph.point.y;
                
                // Check if we're still on the same line (within 0.5pt tolerance)
                let same_line = current_y.map_or(true, |y: f32| (y - glyph_y).abs() < 0.5);
                
                if same_line {
                    current_line_glyphs.push(glyph);
                    current_y = Some(glyph_y);
                } else {
                    // Render the previous line
                    if !current_line_glyphs.is_empty() {
                        render_glyph_line(ops, &current_line_glyphs, &font_id, current_y.unwrap(), page_height, font_manager, current_text_layout);
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
                render_glyph_line(ops, &current_line_glyphs, &font_id, current_y.unwrap(), page_height, font_manager, current_text_layout);
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
                .map(|w| w.inner.to_pixels_internal(0.0, DEFAULT_FONT_SIZE))
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
fn render_glyph_line<T: ParsedFontTrait + 'static, Q: FontLoaderTrait<T>>(
    ops: &mut Vec<Op>,
    glyphs: &[&GlyphInstance],
    font_id: &FontId,
    y_pos: f32,
    page_height: f32,
    _font_manager: &FontManager<T, Q>,
    current_text_layout: &Option<(&UnifiedLayout<T>, LogicalRect)>,
) {
    if glyphs.is_empty() {
        return;
    }

    println!("[bridge] Rendering glyph line with {} glyphs at y={}", glyphs.len(), y_pos);

    // Get the container bounds from TextLayout
    let container_bounds = current_text_layout.as_ref().map(|(_, bounds)| *bounds);
    
    // Set text position to the start of the line
    // Glyphs have positions RELATIVE to their container, so we add container.origin
    let first_x = glyphs[0].point.x;
    let (absolute_x, absolute_y) = if let Some(bounds) = container_bounds {
        println!("[bridge] Using container bounds: origin=({}, {})", bounds.origin.x, bounds.origin.y);
        // y_pos is glyph.point.y (relative to container)
        // Add container origin to get absolute position, then convert to PDF coordinates
        let absolute_x = bounds.origin.x + first_x;
        let absolute_y_layout = bounds.origin.y + y_pos;
        let absolute_y_pdf = page_height - absolute_y_layout;
        (absolute_x, absolute_y_pdf)
    } else {
        println!("[bridge] WARNING: No container bounds, using glyph-relative position");
        (first_x, page_height - y_pos)
    };
    
    ops.push(Op::SetTextCursor {
        pos: crate::graphics::Point::new(Mm(absolute_x * 0.3527777778), Mm(absolute_y * 0.3527777778)),
    });

    // Build glyph-to-unicode mapping from UnifiedLayout
    let mut codepoints: Vec<(u16, char)> = Vec::new();
    
    if let Some((text_layout, _bounds)) = current_text_layout {
        // Build a map from glyph_id to the cluster's complete text
        // This handles ligatures correctly: "fi" ligature glyph maps to "fi" text
        use std::collections::HashMap;
        use azul_layout::text3::cache::ShapedItem;
        
        let mut glyph_to_cluster_text: HashMap<u16, String> = HashMap::new();
        
        // Iterate through all positioned items in the layout
        for positioned_item in &text_layout.items {
            match &positioned_item.item {
                ShapedItem::Cluster(cluster) => {
                    // Map each glyph in the cluster to the cluster's complete text
                    // For ligatures (e.g., "fi" -> single glyph_id=123), cluster.text = "fi"
                    // For normal chars (e.g., "a" -> glyph_id=45), cluster.text = "a"
                    for shaped_glyph in &cluster.glyphs {
                        glyph_to_cluster_text.insert(shaped_glyph.glyph_id, cluster.text.clone());
                    }
                }
                _ => {} // Ignore other item types
            }
        }
        
        println!("[bridge] Built glyph-to-cluster-text map with {} entries", glyph_to_cluster_text.len());
        
        // Build codepoints for ToUnicode CMap
        // For ligatures, we map to the FIRST character (PDF spec limitation)
        for glyph in glyphs {
            let glyph_id = (glyph.index & 0xFFFF) as u16;
            
            if let Some(cluster_text) = glyph_to_cluster_text.get(&glyph_id) {
                let chars: Vec<char> = cluster_text.chars().collect();
                
                if let Some(&first_char) = chars.first() {
                    codepoints.push((glyph_id, first_char));
                    
                    if chars.len() > 1 {
                        println!("[bridge] Ligature: glyph_id={} maps to '{}'", 
                                glyph_id, cluster_text);
                    }
                } else {
                    codepoints.push((glyph_id, '\u{FFFD}'));
                }
            } else {
                codepoints.push((glyph_id, '\u{FFFD}'));
            }
        }
    } else {
        // Fallback: No TextLayout available, use replacement characters
        for glyph in glyphs {
            let glyph_id = (glyph.index & 0xFFFF) as u16;
            codepoints.push((glyph_id, '\u{FFFD}'));
        }
    }

    println!("[bridge] Generated {} codepoints for ToUnicode CMap", codepoints.len());

    if !codepoints.is_empty() {
        ops.push(Op::WriteCodepoints {
            font: font_id.clone(),
            cp: codepoints,
        });
    }
}
