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

    // Process any remaining TextLayout that was collected (should be rare now)
    if let Some((layout, bounds)) = current_text_layout {
        println!("[bridge] Processing final TextLayout with {} items at bounds {:?} (this should be rare)", layout.items.len(), bounds);
        // Note: This will likely be redundant since TextLayouts are processed immediately now
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
            color,
        } => {
            // Extract the UnifiedLayout from the type-erased Arc<dyn Any>
            if let Some(unified_layout) = layout.downcast_ref::<azul_layout::text3::cache::UnifiedLayout<T>>() {
                println!("[bridge] ✓ Found TextLayout with {} items, bounds={:?}", unified_layout.items.len(), bounds);
                
                // Process this TextLayout immediately instead of just storing it
                render_unified_layout_impl(ops, unified_layout, bounds, *color, page_height, font_manager);
                
                // Also update the current text layout for any subsequent processing
                *current_text_layout = Some((unified_layout, *bounds));
            } else {
                println!("[bridge] ✗ Failed to downcast TextLayout");
            }
        }

        DisplayListItem::Text { glyphs, .. } => {
            // IGNORE: Text items are for visual renderers, not PDF generation
            // The azul-layout code pushes TextLayout items BEFORE Text items
            // We only process the TextLayout items which contain the full UnifiedLayout
            if !glyphs.is_empty() {
                println!("[bridge] IGNORING Text item with {} glyphs - use TextLayout instead", glyphs.len());
            }
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
    use azul_layout::text3::cache::ShapedItem;
    use std::collections::HashMap;
    
    println!(
        "[bridge] Rendering unified layout with {} items, bounds={:?}", 
        layout.items.len(), 
        bounds
    );

    // Build a comprehensive map from glyph_id to cluster text for the entire layout
    let mut glyph_to_cluster_text: HashMap<u16, String> = HashMap::new();
    let mut font_hashes_used = std::collections::BTreeSet::new();
    
    // First pass: collect all glyph-to-text mappings and font hashes
    for positioned_item in &layout.items {
        match &positioned_item.item {
            ShapedItem::Cluster(cluster) => {
                for shaped_glyph in &cluster.glyphs {
                    // Store the cluster text for this glyph
                    glyph_to_cluster_text.insert(shaped_glyph.glyph_id, cluster.text.clone());
                    font_hashes_used.insert(shaped_glyph.font.get_hash());
                }
            }
            ShapedItem::CombinedBlock { glyphs, .. } => {
                for shaped_glyph in glyphs {
                    // For combined blocks, we might not have cluster text
                    // Use a placeholder or the glyph's corresponding character
                    glyph_to_cluster_text.insert(shaped_glyph.glyph_id, "?".to_string());
                    font_hashes_used.insert(shaped_glyph.font.get_hash());
                }
            }
            _ => {
                // Ignore other item types (non-text)
            }
        }
    }
    
    println!(
        "[bridge] Built glyph-to-cluster-text map with {} entries, {} fonts used", 
        glyph_to_cluster_text.len(),
        font_hashes_used.len()
    );

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor {
        col: convert_color(&color),
    });

    // Store the text layout origin for absolute positioning
    let text_origin_x = bounds.origin.x;
    let text_origin_y = bounds.origin.y;

    // Process all text items using absolute positioning with TextMatrix (Tm operator)
    let mut current_font_hash: Option<u64> = None;
    
    for positioned_item in &layout.items {
        match &positioned_item.item {
            ShapedItem::Cluster(cluster) => {
                if cluster.glyphs.is_empty() {
                    continue;
                }
                
                let font_hash = cluster.glyphs[0].font.get_hash();
                
                // Set font if it changed
                if current_font_hash != Some(font_hash) {
                    let font_id = FontId(format!("F{}", font_hash));
                    let font_size = cluster.glyphs[0].style.font_size_px;
                    
                    ops.push(Op::SetFont {
                        font: crate::ops::PdfFontHandle::External(font_id.clone()),
                        size: Pt(font_size),
                    });
                    current_font_hash = Some(font_hash);
                }
                
                // Convert cluster glyphs to codepoints with CID information
                let mut codepoints: Vec<crate::text::Codepoint> = Vec::new();
                
                for shaped_glyph in &cluster.glyphs {
                    let cid = glyph_to_cluster_text.get(&shaped_glyph.glyph_id).cloned();
                    codepoints.push(crate::text::Codepoint {
                        gid: shaped_glyph.glyph_id,
                        offset: 0.0, // Layout engine handles positioning
                        cid,
                    });
                }
                
                // Calculate absolute position relative to text origin
                let absolute_x = text_origin_x + positioned_item.position.x;
                let absolute_y_layout = text_origin_y + positioned_item.position.y;
                let absolute_y_pdf = page_height - absolute_y_layout;
                
                // Use TextMatrix (Tm operator) for absolute positioning
                // This sets the text matrix to position + identity transform
                ops.push(Op::SetTextMatrix {
                    matrix: crate::matrix::TextMatrix::Raw([
                        1.0, // a: Scale X (no scaling)
                        0.0, // b: Skew X 
                        0.0, // c: Skew Y
                        1.0, // d: Scale Y (no scaling)
                        absolute_x * 0.3527777778, // e: Translate X (px to points)
                        absolute_y_pdf * 0.3527777778, // f: Translate Y (px to points, flipped)
                    ]),
                });
                
                ops.push(Op::ShowText {
                    items: vec![crate::text::TextItem::GlyphIds(codepoints)],
                });
            }
            _ => {
                // Handle other shaped item types if needed
            }
        }
    }

    ops.push(Op::EndTextSection);
}
