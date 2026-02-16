//! Border and border-radius rendering utilities for PDF generation.
//!
//! This module provides comprehensive border rendering including:
//! - All four sides (top, right, bottom, left) rendered individually
//! - Border-radius support with proper corner curves
//! - Border styles (solid, dashed, dotted, etc.)
//! - Reusable path generation for clipping rectangles

use azul_core::geom::{LogicalRect, LogicalPosition, LogicalSize};
use azul_css::props::basic::{pixel::DEFAULT_FONT_SIZE, ColorU};
use azul_layout::solver3::display_list::{StyleBorderColors, StyleBorderStyles, StyleBorderWidths};
use azul_css::props::style::border_radius::StyleBorderRadius;
use crate::{Color, Mm, Op, Pt, Rgb, LineCapStyle, LineDashPattern};

/// Create a `graphics::Point` from PDF-pt coordinates.
#[inline]
fn pt_point(x: f32, y: f32) -> crate::graphics::Point {
    crate::graphics::Point::new(Mm::from(Pt(x)), Mm::from(Pt(y)))
}

/// Convert azul ColorU to printpdf Color
pub fn convert_color(color: &ColorU) -> Color {
    Color::Rgb(Rgb {
        r: color.r as f32 / 255.0,
        g: color.g as f32 / 255.0,
        b: color.b as f32 / 255.0,
        icc_profile: None,
    })
}

/// Represents a corner of a rectangle
#[derive(Debug, Clone, Copy)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

/// Represents a side of a border
#[derive(Debug, Clone, Copy)]
pub enum BorderSide {
    Top,
    Right,
    Bottom,
    Left,
}

/// Border rendering configuration
#[derive(Debug, Clone)]
pub struct BorderConfig {
    pub bounds: LogicalRect,
    pub widths: BorderWidths,
    pub colors: BorderColors,
    pub styles: BorderStyles,
    pub radii: BorderRadii,
    pub page_height: f32,
    /// Left margin offset in points
    pub margin_left: f32,
    /// Top margin offset in points  
    pub margin_top: f32,
}

/// Processed border widths (in pixels)
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Processed border colors
#[derive(Debug, Clone, Copy)]
pub struct BorderColors {
    pub top: ColorU,
    pub right: ColorU,
    pub bottom: ColorU,
    pub left: ColorU,
}

/// Processed border styles
#[derive(Debug, Clone, Copy)]
pub struct BorderStyles {
    pub top: BorderStyleType,
    pub right: BorderStyleType,
    pub bottom: BorderStyleType,
    pub left: BorderStyleType,
}

/// Border radii for all four corners (in pixels)
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderRadii {
    pub top_left: (f32, f32),     // (horizontal, vertical)
    pub top_right: (f32, f32),
    pub bottom_right: (f32, f32),
    pub bottom_left: (f32, f32),
}

/// Border style types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyleType {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl Default for BorderStyleType {
    fn default() -> Self {
        BorderStyleType::Solid
    }
}

impl Default for BorderColors {
    fn default() -> Self {
        BorderColors {
            top: ColorU { r: 0, g: 0, b: 0, a: 255 },
            right: ColorU { r: 0, g: 0, b: 0, a: 255 },
            bottom: ColorU { r: 0, g: 0, b: 0, a: 255 },
            left: ColorU { r: 0, g: 0, b: 0, a: 255 },
        }
    }
}

impl Default for BorderStyles {
    fn default() -> Self {
        BorderStyles {
            top: BorderStyleType::Solid,
            right: BorderStyleType::Solid,
            bottom: BorderStyleType::Solid,
            left: BorderStyleType::Solid,
        }
    }
}

/// Extract border widths from CSS property values
pub fn extract_border_widths(widths: &StyleBorderWidths) -> BorderWidths {
    BorderWidths {
        top: widths
            .top
            .and_then(|w| w.get_property().cloned())
            .map(|w| w.inner.to_pixels_internal(0.0, DEFAULT_FONT_SIZE))
            .unwrap_or(0.0),
        right: widths
            .right
            .and_then(|w| w.get_property().cloned())
            .map(|w| w.inner.to_pixels_internal(0.0, DEFAULT_FONT_SIZE))
            .unwrap_or(0.0),
        bottom: widths
            .bottom
            .and_then(|w| w.get_property().cloned())
            .map(|w| w.inner.to_pixels_internal(0.0, DEFAULT_FONT_SIZE))
            .unwrap_or(0.0),
        left: widths
            .left
            .and_then(|w| w.get_property().cloned())
            .map(|w| w.inner.to_pixels_internal(0.0, DEFAULT_FONT_SIZE))
            .unwrap_or(0.0),
    }
}

/// Extract border colors from CSS property values
pub fn extract_border_colors(colors: &StyleBorderColors) -> BorderColors {
    BorderColors {
        top: colors
            .top
            .and_then(|c| c.get_property().cloned())
            .map(|c| c.inner)
            .unwrap_or(ColorU { r: 0, g: 0, b: 0, a: 255 }),
        right: colors
            .right
            .and_then(|c| c.get_property().cloned())
            .map(|c| c.inner)
            .unwrap_or(ColorU { r: 0, g: 0, b: 0, a: 255 }),
        bottom: colors
            .bottom
            .and_then(|c| c.get_property().cloned())
            .map(|c| c.inner)
            .unwrap_or(ColorU { r: 0, g: 0, b: 0, a: 255 }),
        left: colors
            .left
            .and_then(|c| c.get_property().cloned())
            .map(|c| c.inner)
            .unwrap_or(ColorU { r: 0, g: 0, b: 0, a: 255 }),
    }
}

/// Extract border styles from CSS property values
pub fn extract_border_styles(styles: &StyleBorderStyles) -> BorderStyles {
    use azul_css::props::style::BorderStyle;
    
    let convert_top = |s: Option<azul_css::css::CssPropertyValue<azul_css::props::style::StyleBorderTopStyle>>| -> BorderStyleType {
        s.and_then(|v| v.get_property().cloned())
            .map(|v| match v.inner {
                BorderStyle::None => BorderStyleType::None,
                BorderStyle::Solid => BorderStyleType::Solid,
                BorderStyle::Double => BorderStyleType::Double,
                BorderStyle::Dotted => BorderStyleType::Dotted,
                BorderStyle::Dashed => BorderStyleType::Dashed,
                BorderStyle::Hidden => BorderStyleType::None,
                BorderStyle::Groove => BorderStyleType::Groove,
                BorderStyle::Ridge => BorderStyleType::Ridge,
                BorderStyle::Inset => BorderStyleType::Inset,
                BorderStyle::Outset => BorderStyleType::Outset,
            })
            .unwrap_or(BorderStyleType::Solid)
    };
    
    let convert_right = |s: Option<azul_css::css::CssPropertyValue<azul_css::props::style::StyleBorderRightStyle>>| -> BorderStyleType {
        s.and_then(|v| v.get_property().cloned())
            .map(|v| match v.inner {
                BorderStyle::None => BorderStyleType::None,
                BorderStyle::Solid => BorderStyleType::Solid,
                BorderStyle::Double => BorderStyleType::Double,
                BorderStyle::Dotted => BorderStyleType::Dotted,
                BorderStyle::Dashed => BorderStyleType::Dashed,
                BorderStyle::Hidden => BorderStyleType::None,
                BorderStyle::Groove => BorderStyleType::Groove,
                BorderStyle::Ridge => BorderStyleType::Ridge,
                BorderStyle::Inset => BorderStyleType::Inset,
                BorderStyle::Outset => BorderStyleType::Outset,
            })
            .unwrap_or(BorderStyleType::Solid)
    };
    
    let convert_bottom = |s: Option<azul_css::css::CssPropertyValue<azul_css::props::style::StyleBorderBottomStyle>>| -> BorderStyleType {
        s.and_then(|v| v.get_property().cloned())
            .map(|v| match v.inner {
                BorderStyle::None => BorderStyleType::None,
                BorderStyle::Solid => BorderStyleType::Solid,
                BorderStyle::Double => BorderStyleType::Double,
                BorderStyle::Dotted => BorderStyleType::Dotted,
                BorderStyle::Dashed => BorderStyleType::Dashed,
                BorderStyle::Hidden => BorderStyleType::None,
                BorderStyle::Groove => BorderStyleType::Groove,
                BorderStyle::Ridge => BorderStyleType::Ridge,
                BorderStyle::Inset => BorderStyleType::Inset,
                BorderStyle::Outset => BorderStyleType::Outset,
            })
            .unwrap_or(BorderStyleType::Solid)
    };
    
    let convert_left = |s: Option<azul_css::css::CssPropertyValue<azul_css::props::style::StyleBorderLeftStyle>>| -> BorderStyleType {
        s.and_then(|v| v.get_property().cloned())
            .map(|v| match v.inner {
                BorderStyle::None => BorderStyleType::None,
                BorderStyle::Solid => BorderStyleType::Solid,
                BorderStyle::Double => BorderStyleType::Double,
                BorderStyle::Dotted => BorderStyleType::Dotted,
                BorderStyle::Dashed => BorderStyleType::Dashed,
                BorderStyle::Hidden => BorderStyleType::None,
                BorderStyle::Groove => BorderStyleType::Groove,
                BorderStyle::Ridge => BorderStyleType::Ridge,
                BorderStyle::Inset => BorderStyleType::Inset,
                BorderStyle::Outset => BorderStyleType::Outset,
            })
            .unwrap_or(BorderStyleType::Solid)
    };

    BorderStyles {
        top: convert_top(styles.top),
        right: convert_right(styles.right),
        bottom: convert_bottom(styles.bottom),
        left: convert_left(styles.left),
    }
}

/// Extract border radii from CSS property values
pub fn extract_border_radii(border_radius: &StyleBorderRadius) -> BorderRadii {
    let tl = border_radius.top_left.to_pixels_internal(0.0, DEFAULT_FONT_SIZE);
    let tr = border_radius.top_right.to_pixels_internal(0.0, DEFAULT_FONT_SIZE);
    let br = border_radius.bottom_right.to_pixels_internal(0.0, DEFAULT_FONT_SIZE);
    let bl = border_radius.bottom_left.to_pixels_internal(0.0, DEFAULT_FONT_SIZE);
    
    BorderRadii {
        top_left: (tl, tl),      // Use same value for horizontal and vertical
        top_right: (tr, tr),
        bottom_right: (br, br),
        bottom_left: (bl, bl),
    }
}

/// Create rounded rectangle path with margin support
pub fn create_rounded_rect_path_with_margins(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radii: &BorderRadii,
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
) -> Vec<crate::graphics::LinePoint> {
    create_rounded_rect_path_with_margins_internal(x, y, width, height, radii, page_height, margin_left, margin_top)
}

/// Internal implementation of rounded rectangle path generation with margin support
fn create_rounded_rect_path_with_margins_internal(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radii: &BorderRadii,
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
) -> Vec<crate::graphics::LinePoint> {
    const KAPPA: f32 = 0.5522847498;
    
    // Apply margins: add margin_left to X, subtract margin_top from Y before PDF transformation
    let x = x + margin_left;
    
    // Convert to PDF coordinates (bottom-left origin) with margin_top
    // Layout Y is from top, PDF Y is from bottom
    // PDF_Y = page_height - layout_y - height - margin_top
    let y_pdf = page_height - y - height - margin_top;
    
    // Extract corner radii
    let tl_x = radii.top_left.0.min(width / 2.0).max(0.0);
    let tl_y = radii.top_left.1.min(height / 2.0).max(0.0);
    let tr_x = radii.top_right.0.min(width / 2.0).max(0.0);
    let tr_y = radii.top_right.1.min(height / 2.0).max(0.0);
    let br_x = radii.bottom_right.0.min(width / 2.0).max(0.0);
    let br_y = radii.bottom_right.1.min(height / 2.0).max(0.0);
    let bl_x = radii.bottom_left.0.min(width / 2.0).max(0.0);
    let bl_y = radii.bottom_left.1.min(height / 2.0).max(0.0);
    
    let mut points = Vec::new();
    
    // Start at top-left corner (after radius)
    let start_x = x + tl_x;
    let start_y = y_pdf + height;
    points.push(crate::graphics::LinePoint {
        p: pt_point(start_x, start_y),
        bezier: false,
    });
    
    // Top edge to top-right corner
    let top_right_start_x = x + width - tr_x;
    points.push(crate::graphics::LinePoint {
        p: pt_point(top_right_start_x, y_pdf + height),
        bezier: false,
    });
    
    // Top-right corner curve (clockwise)
    if tr_x > 0.0 || tr_y > 0.0 {
        let cp1_x = top_right_start_x + tr_x * KAPPA;
        let cp1_y = y_pdf + height;
        let cp2_x = x + width;
        let cp2_y = y_pdf + height - tr_y * (1.0 - KAPPA);
        let end_x = x + width;
        let end_y = y_pdf + height - tr_y;
        
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp1_x, cp1_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp2_x, cp2_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(end_x, end_y),
            bezier: false,
        });
    }
    
    // Right edge to bottom-right corner
    let bottom_right_start_y = y_pdf + br_y;
    points.push(crate::graphics::LinePoint {
        p: pt_point(x + width, bottom_right_start_y),
        bezier: false,
    });
    
    // Bottom-right corner curve
    if br_x > 0.0 || br_y > 0.0 {
        let cp1_x = x + width;
        let cp1_y = y_pdf + br_y * KAPPA;
        let cp2_x = x + width - br_x * (1.0 - KAPPA);
        let cp2_y = y_pdf;
        let end_x = x + width - br_x;
        let end_y = y_pdf;
        
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp1_x, cp1_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp2_x, cp2_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(end_x, end_y),
            bezier: false,
        });
    }
    
    // Bottom edge to bottom-left corner
    let bottom_left_start_x = x + bl_x;
    points.push(crate::graphics::LinePoint {
        p: pt_point(bottom_left_start_x, y_pdf),
        bezier: false,
    });
    
    // Bottom-left corner curve
    if bl_x > 0.0 || bl_y > 0.0 {
        let cp1_x = x + bl_x * KAPPA;
        let cp1_y = y_pdf;
        let cp2_x = x;
        let cp2_y = y_pdf + bl_y * (1.0 - KAPPA);
        let end_x = x;
        let end_y = y_pdf + bl_y;
        
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp1_x, cp1_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp2_x, cp2_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(end_x, end_y),
            bezier: false,
        });
    }
    
    // Left edge back to start
    points.push(crate::graphics::LinePoint {
        p: pt_point(x, y_pdf + height - tl_y),
        bezier: false,
    });
    
    // Top-left corner curve
    if tl_x > 0.0 || tl_y > 0.0 {
        let cp1_x = x;
        let cp1_y = y_pdf + height - tl_y * KAPPA;
        let cp2_x = x + tl_x * (1.0 - KAPPA);
        let cp2_y = y_pdf + height;
        let end_x = x + tl_x;
        let end_y = y_pdf + height;
        
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp1_x, cp1_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(cp2_x, cp2_y),
            bezier: true,
        });
        points.push(crate::graphics::LinePoint {
            p: pt_point(end_x, end_y),
            bezier: false,
        });
    }
    
    points
}

/// Check if any radius is non-zero
fn has_border_radius(radii: &BorderRadii) -> bool {
    radii.top_left.0 > 0.0 || radii.top_left.1 > 0.0
        || radii.top_right.0 > 0.0 || radii.top_right.1 > 0.0
        || radii.bottom_right.0 > 0.0 || radii.bottom_right.1 > 0.0
        || radii.bottom_left.0 > 0.0 || radii.bottom_left.1 > 0.0
}

/// Render all four borders with full support for border-radius and styles
/// 
/// NOTE: This function expects bounds in CSS coordinates (top-left origin).
/// The Y-coordinate conversion to PDF space (bottom-left origin) happens internally.
/// Margins are applied to shift the border within the page.
pub fn render_border(ops: &mut Vec<Op>, config: &BorderConfig) {
    let widths = &config.widths;
    let colors = &config.colors;
    let styles = &config.styles;
    let radii = &config.radii;
    let bounds = &config.bounds;
    let margin_left = config.margin_left;
    let margin_top = config.margin_top;

    // Check if all sides are identical (common case optimization)
    let all_same = widths.top == widths.right && widths.top == widths.bottom && widths.top == widths.left
        && colors.top == colors.right && colors.top == colors.bottom && colors.top == colors.left
        && styles.top == styles.right && styles.top == styles.bottom && styles.top == styles.left
        && styles.top == BorderStyleType::Solid;

    if all_same && widths.top > 0.0 && colors.top.a > 0 {
        // Optimized path: render as a single stroked rectangle (with or without border-radius)
        render_unified_border(ops, bounds, widths.top, colors.top, radii, config.page_height, margin_left, margin_top);
        return;
    }

    // Render each side individually
    if widths.top > 0.0 && styles.top != BorderStyleType::None && colors.top.a > 0 {
        render_border_side(ops, BorderSide::Top, bounds, widths, colors, styles, radii, config.page_height, margin_left, margin_top);
    }
    if widths.right > 0.0 && styles.right != BorderStyleType::None && colors.right.a > 0 {
        render_border_side(ops, BorderSide::Right, bounds, widths, colors, styles, radii, config.page_height, margin_left, margin_top);
    }
    if widths.bottom > 0.0 && styles.bottom != BorderStyleType::None && colors.bottom.a > 0 {
        render_border_side(ops, BorderSide::Bottom, bounds, widths, colors, styles, radii, config.page_height, margin_left, margin_top);
    }
    if widths.left > 0.0 && styles.left != BorderStyleType::None && colors.left.a > 0 {
        render_border_side(ops, BorderSide::Left, bounds, widths, colors, styles, radii, config.page_height, margin_left, margin_top);
    }
}

/// Render a border as a single stroked rectangle (optimization for uniform borders)
fn render_unified_border(
    ops: &mut Vec<Op>,
    bounds: &LogicalRect,
    width: f32,
    color: ColorU,
    radii: &BorderRadii,
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
) {
    ops.push(Op::SaveGraphicsState);
    
    // Set color and width
    ops.push(Op::SetOutlineColor { col: convert_color(&color) });
    ops.push(Op::SetOutlineThickness { pt: Pt(width) });
    
    // Check if we have border radius
    if has_border_radius(radii) {
        // Adjust bounds for stroke centering: the stroke is centered on the path,
        // so we need to inset by half the width to keep the border inside
        let half_width = width / 2.0;
        let adj_x = bounds.origin.x + half_width;
        let adj_y = bounds.origin.y + half_width;
        let adj_width = bounds.size.width - width;
        let adj_height = bounds.size.height - width;
        
        // Also adjust radii to account for the inset
        let adj_radii = BorderRadii {
            top_left: (
                (radii.top_left.0 - half_width).max(0.0),
                (radii.top_left.1 - half_width).max(0.0)
            ),
            top_right: (
                (radii.top_right.0 - half_width).max(0.0),
                (radii.top_right.1 - half_width).max(0.0)
            ),
            bottom_right: (
                (radii.bottom_right.0 - half_width).max(0.0),
                (radii.bottom_right.1 - half_width).max(0.0)
            ),
            bottom_left: (
                (radii.bottom_left.0 - half_width).max(0.0),
                (radii.bottom_left.1 - half_width).max(0.0)
            ),
        };
        
        let points = create_rounded_rect_path_with_margins_internal(
            adj_x,
            adj_y,
            adj_width,
            adj_height,
            &adj_radii,
            page_height,
            margin_left,
            margin_top,
        );
        
        let polygon = crate::graphics::Polygon {
            rings: vec![crate::graphics::PolygonRing { points }],
            mode: crate::graphics::PaintMode::Stroke,
            winding_order: crate::graphics::WindingOrder::NonZero,
        };
        
        ops.push(Op::DrawPolygon { polygon });
    } else {
        // Simple rectangle without border radius
        // Convert to PDF coordinate space (bottom-left origin) with margins
        let y = page_height - bounds.origin.y - bounds.size.height - margin_top;
        
        // Adjust for stroke centering and apply left margin
        let half_width = width / 2.0;
        let x = bounds.origin.x + half_width + margin_left;
        let y_adj = y + half_width;
        let w = bounds.size.width - width;
        let h = bounds.size.height - width;
        
        let polygon = crate::graphics::Polygon {
            rings: vec![crate::graphics::PolygonRing {
                points: vec![
                    crate::graphics::LinePoint {
                        p: pt_point(x, y_adj),
                        bezier: false,
                    },
                    crate::graphics::LinePoint {
                        p: pt_point(x + w, y_adj),
                        bezier: false,
                    },
                    crate::graphics::LinePoint {
                        p: pt_point(x + w, y_adj + h),
                        bezier: false,
                    },
                    crate::graphics::LinePoint {
                        p: pt_point(x, y_adj + h),
                        bezier: false,
                    },
                ],
            }],
            mode: crate::graphics::PaintMode::Stroke,
            winding_order: crate::graphics::WindingOrder::NonZero,
        };
        
        ops.push(Op::DrawPolygon { polygon });
    }
    
    ops.push(Op::RestoreGraphicsState);
}

/// Render a single border side
fn render_border_side(
    ops: &mut Vec<Op>,
    side: BorderSide,
    bounds: &LogicalRect,
    widths: &BorderWidths,
    colors: &BorderColors,
    styles: &BorderStyles,
    radii: &BorderRadii,
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
) {
    let (width, color, style) = match side {
        BorderSide::Top => (widths.top, colors.top, styles.top),
        BorderSide::Right => (widths.right, colors.right, styles.right),
        BorderSide::Bottom => (widths.bottom, colors.bottom, styles.bottom),
        BorderSide::Left => (widths.left, colors.left, styles.left),
    };

    ops.push(Op::SaveGraphicsState);

    // Set color, width, and round line caps for smooth corner connections
    ops.push(Op::SetOutlineColor { col: convert_color(&color) });
    ops.push(Op::SetOutlineThickness { pt: Pt(width) });
    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });

    // Handle different border styles
    match style {
        BorderStyleType::Solid => {
            render_solid_border_side(ops, side, bounds, widths, radii, page_height, margin_left, margin_top);
        }
        BorderStyleType::Dashed => {
            render_dashed_border_side(ops, side, bounds, widths, radii, page_height, width, margin_left, margin_top);
        }
        BorderStyleType::Dotted => {
            render_dotted_border_side(ops, side, bounds, widths, radii, page_height, width, margin_left, margin_top);
        }
        BorderStyleType::Double => {
            render_double_border_side(ops, side, bounds, widths, radii, page_height, width, margin_left, margin_top);
        }
        _ => {
            // For groove, ridge, inset, outset - fall back to solid for now
            render_solid_border_side(ops, side, bounds, widths, radii, page_height, margin_left, margin_top);
        }
    }

    ops.push(Op::RestoreGraphicsState);
}

/// Render a solid border side with border-radius support
/// 
/// NOTE: bounds are in CSS coordinates (top-left origin).
/// Y-coordinate conversion to PDF space happens here.
fn render_solid_border_side(
    ops: &mut Vec<Op>,
    side: BorderSide,
    bounds: &LogicalRect,
    widths: &BorderWidths,
    radii: &BorderRadii,
    page_height: f32,
    margin_left: f32,
    margin_top: f32,
) {
    // Apply margin_left to X coordinate
    let x = bounds.origin.x + margin_left;
    let w = bounds.size.width;
    
    // Convert CSS Y (top-left origin) to PDF Y (bottom-left origin) with margin_top
    let pdf_y_bottom = page_height - bounds.origin.y - bounds.size.height - margin_top;
    let pdf_y_top = page_height - bounds.origin.y - margin_top;
    
    // For corners with radius, we pass the corner position to add_corner_curve
    // which will handle the 45° split. The straight part coordinates are calculated
    // to connect properly with the corner curves.
    
    let line = match side {
        BorderSide::Top => {
            // Top border runs from left to right at pdf_y_top
            // Start corner: TopLeft, End corner: TopRight
            let has_start_radius = radii.top_left.0 > 0.1 || radii.top_left.1 > 0.1;
            let has_end_radius = radii.top_right.0 > 0.1 || radii.top_right.1 > 0.1;
            
            // For the straight part, we start after the left radius and end before the right radius
            let straight_start_x = x + radii.top_left.0;
            let straight_end_x = x + w - radii.top_right.0;
            
            create_line_with_corners(
                straight_start_x, pdf_y_top, straight_end_x, pdf_y_top,
                radii.top_left, radii.top_right,
                Corner::TopLeft, Corner::TopRight,
                widths.left, widths.right, widths.top,
                has_start_radius, has_end_radius,
            )
        }
        BorderSide::Right => {
            // Right border runs from top to bottom at x + w
            // Start corner: TopRight, End corner: BottomRight
            let has_start_radius = radii.top_right.0 > 0.1 || radii.top_right.1 > 0.1;
            let has_end_radius = radii.bottom_right.0 > 0.1 || radii.bottom_right.1 > 0.1;
            
            let straight_start_y = pdf_y_top - radii.top_right.1;
            let straight_end_y = pdf_y_bottom + radii.bottom_right.1;
            
            create_line_with_corners(
                x + w, straight_start_y, x + w, straight_end_y,
                radii.top_right, radii.bottom_right,
                Corner::TopRight, Corner::BottomRight,
                widths.top, widths.bottom, widths.right,
                has_start_radius, has_end_radius,
            )
        }
        BorderSide::Bottom => {
            // Bottom border runs from right to left at pdf_y_bottom
            // Start corner: BottomRight, End corner: BottomLeft
            let has_start_radius = radii.bottom_right.0 > 0.1 || radii.bottom_right.1 > 0.1;
            let has_end_radius = radii.bottom_left.0 > 0.1 || radii.bottom_left.1 > 0.1;
            
            let straight_start_x = x + w - radii.bottom_right.0;
            let straight_end_x = x + radii.bottom_left.0;
            
            create_line_with_corners(
                straight_start_x, pdf_y_bottom, straight_end_x, pdf_y_bottom,
                radii.bottom_right, radii.bottom_left,
                Corner::BottomRight, Corner::BottomLeft,
                widths.right, widths.left, widths.bottom,
                has_start_radius, has_end_radius,
            )
        }
        BorderSide::Left => {
            // Left border runs from bottom to top at x
            // Start corner: BottomLeft, End corner: TopLeft
            let has_start_radius = radii.bottom_left.0 > 0.1 || radii.bottom_left.1 > 0.1;
            let has_end_radius = radii.top_left.0 > 0.1 || radii.top_left.1 > 0.1;
            
            let straight_start_y = pdf_y_bottom + radii.bottom_left.1;
            let straight_end_y = pdf_y_top - radii.top_left.1;
            
            create_line_with_corners(
                x, straight_start_y, x, straight_end_y,
                radii.bottom_left, radii.top_left,
                Corner::BottomLeft, Corner::TopLeft,
                widths.bottom, widths.top, widths.left,
                has_start_radius, has_end_radius,
            )
        }
    };

    ops.push(Op::DrawLine { line });
}

/// Create a line with optional curved corners at the 45° split points
fn create_line_with_corners(
    start_x: f32, start_y: f32,
    end_x: f32, end_y: f32,
    start_radius: (f32, f32),
    end_radius: (f32, f32),
    start_corner: Corner,
    end_corner: Corner,
    start_perpendicular_width: f32,
    end_perpendicular_width: f32,
    parallel_width: f32,
    has_start_radius: bool,
    has_end_radius: bool,
) -> crate::graphics::Line {
    let mut points = Vec::new();

    // Add start corner curve (second half of the arc, from 45° to edge)
    if has_start_radius {
        add_corner_curve(&mut points, start_x, start_y, start_radius, start_corner, start_perpendicular_width, parallel_width, true);
    } else {
        points.push(crate::graphics::LinePoint {
            p: pt_point(start_x, start_y),
            bezier: false,
        });
    }

    // Add end corner curve (first half of the arc, from edge to 45°)
    if has_end_radius {
        add_corner_curve(&mut points, end_x, end_y, end_radius, end_corner, end_perpendicular_width, parallel_width, false);
    } else {
        points.push(crate::graphics::LinePoint {
            p: pt_point(end_x, end_y),
            bezier: false,
        });
    }

    crate::graphics::Line {
        points,
        is_closed: false,
    }
}

/// Add a corner arc to the line points using regular points (sin/cos).
/// 
/// Each border side renders HALF of the corner arc (split at 45°).
/// - `is_start = true`: This is the START corner of the side, so we render 
///   from the 45° midpoint TO the side's straight edge
/// - `is_start = false`: This is the END corner of the side, so we render
///   from the side's straight edge TO the 45° midpoint
///
/// The `x, y` coordinates are the point where the straight part of this border
/// side begins/ends (i.e., at the tangent point of the radius).
///
/// Uses 1 point per pixel of the larger radius for smooth curves.
fn add_corner_curve(
    points: &mut Vec<crate::graphics::LinePoint>,
    x: f32, y: f32,
    radius: (f32, f32),
    corner: Corner,
    _perpendicular_width: f32,
    _parallel_width: f32,
    is_start: bool,
) {
    let rx = radius.0;
    let ry = radius.1;
    
    if rx < 0.1 && ry < 0.1 {
        points.push(crate::graphics::LinePoint {
            p: pt_point(x, y),
            bezier: false,
        });
        return;
    }
    
    // Number of points = 1 per pixel of the larger radius, minimum 3
    let max_radius = rx.max(ry);
    let num_points = (max_radius as usize).max(3);
    
    // x, y is the tangent point where the straight border meets the arc.
    // We need to calculate the center of the arc and the angle range.
    //
    // For each corner, we draw a 45° arc. The tangent point is where the
    // straight border edge meets the curve.
    //
    // PDF coordinates: Y goes UP (bottom-left origin)
    // Angles: 0° = right (+X), 90° = up (+Y), 180° = left (-X), 270° = down (-Y)
    
    let (center_x, center_y, start_angle, end_angle) = match corner {
        Corner::TopLeft => {
            // TopLeft corner: arc from left side (180°) to top side (90°)
            // 
            // Tangent points:
            //   - Top tangent (90°): (cx, cy + ry)
            //   - Left tangent (180°): (cx - rx, cy)
            // 
            // is_start=true (TOP border): x,y is the top tangent point
            //   So: x = cx, y = cy + ry → cx = x, cy = y - ry
            //   We draw from 135° to 90°
            // is_start=false (LEFT border): x,y is the left tangent point
            //   So: x = cx - rx, y = cy → cx = x + rx, cy = y
            //   We draw from 180° to 135°
            if is_start {
                (x, y - ry, 135.0_f32.to_radians(), 90.0_f32.to_radians())
            } else {
                (x + rx, y, 180.0_f32.to_radians(), 135.0_f32.to_radians())
            }
        }
        Corner::TopRight => {
            // TopRight corner: arc from top side (90°) to right side (0°)
            // 
            // Tangent points:
            //   - Top tangent (90°): (cx, cy + ry)
            //   - Right tangent (0°): (cx + rx, cy)
            // 
            // is_start=true (RIGHT border): x,y is the right tangent point
            //   So: x = cx + rx, y = cy → cx = x - rx, cy = y
            //   We draw from 45° to 0°
            // is_start=false (TOP border): x,y is the top tangent point  
            //   So: x = cx, y = cy + ry → cx = x, cy = y - ry
            //   We draw from 90° to 45°
            if is_start {
                (x - rx, y, 45.0_f32.to_radians(), 0.0_f32.to_radians())
            } else {
                (x, y - ry, 90.0_f32.to_radians(), 45.0_f32.to_radians())
            }
        }
        Corner::BottomRight => {
            // BottomRight corner: arc from right side (0°/360°) to bottom side (270°)
            // 
            // Tangent points:
            //   - Right tangent (0°/360°): (cx + rx, cy)
            //   - Bottom tangent (270°): (cx, cy - ry)
            // 
            // is_start=true (BOTTOM border): x,y is the bottom tangent point
            //   So: x = cx, y = cy - ry → cx = x, cy = y + ry
            //   We draw from 315° to 270°
            // is_start=false (RIGHT border): x,y is the right tangent point
            //   So: x = cx + rx, y = cy → cx = x - rx, cy = y
            //   We draw from 360° to 315°
            if is_start {
                (x, y + ry, 315.0_f32.to_radians(), 270.0_f32.to_radians())
            } else {
                (x - rx, y, 360.0_f32.to_radians(), 315.0_f32.to_radians())
            }
        }
        Corner::BottomLeft => {
            // BottomLeft corner: arc from bottom side (270°) to left side (180°)
            // 
            // Tangent points:
            //   - Bottom tangent (270°): (cx, cy - ry)
            //   - Left tangent (180°): (cx - rx, cy)
            // 
            // is_start=true (LEFT border): x,y is the left tangent point
            //   So: x = cx - rx, y = cy → cx = x + rx, cy = y
            //   We draw from 225° to 180°
            // is_start=false (BOTTOM border): x,y is the bottom tangent point
            //   So: x = cx, y = cy - ry → cx = x, cy = y + ry
            //   We draw from 270° to 225°
            if is_start {
                (x + rx, y, 225.0_f32.to_radians(), 180.0_f32.to_radians())
            } else {
                (x, y + ry, 270.0_f32.to_radians(), 225.0_f32.to_radians())
            }
        }
    };
    
    // Generate points along the arc
    let angle_step = (end_angle - start_angle) / (num_points as f32);
    
    for i in 0..=num_points {
        let angle = start_angle + angle_step * (i as f32);
        let px = center_x + rx * angle.cos();
        let py = center_y + ry * angle.sin();
        
        points.push(crate::graphics::LinePoint {
            p: pt_point(px, py),
            bezier: false,
        });
    }
}

/// Render a dashed border side
fn render_dashed_border_side(
    ops: &mut Vec<Op>,
    side: BorderSide,
    bounds: &LogicalRect,
    widths: &BorderWidths,
    radii: &BorderRadii,
    page_height: f32,
    width: f32,
    margin_left: f32,
    margin_top: f32,
) {
    // Set dash pattern
    ops.push(Op::SetLineDashPattern {
        dash: LineDashPattern {
            offset: 0,
            dash_1: Some((width * 3.0) as i64),
            gap_1: Some((width * 2.0) as i64),
            dash_2: None,
            gap_2: None,
            dash_3: None,
            gap_3: None,
        },
    });
    
    render_solid_border_side(ops, side, bounds, widths, radii, page_height, margin_left, margin_top);
    
    // Reset dash pattern
    ops.push(Op::SetLineDashPattern {
        dash: LineDashPattern {
            offset: 0,
            dash_1: None,
            gap_1: None,
            dash_2: None,
            gap_2: None,
            dash_3: None,
            gap_3: None,
        },
    });
}

/// Render a dotted border side
fn render_dotted_border_side(
    ops: &mut Vec<Op>,
    side: BorderSide,
    bounds: &LogicalRect,
    widths: &BorderWidths,
    radii: &BorderRadii,
    page_height: f32,
    width: f32,
    margin_left: f32,
    margin_top: f32,
) {
    // Set dot pattern (dash = width, gap = width)
    ops.push(Op::SetLineDashPattern {
        dash: LineDashPattern {
            offset: 0,
            dash_1: Some(width as i64),
            gap_1: Some(width as i64),
            dash_2: None,
            gap_2: None,
            dash_3: None,
            gap_3: None,
        },
    });
    
    // Set round line cap for dots
    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });
    
    render_solid_border_side(ops, side, bounds, widths, radii, page_height, margin_left, margin_top);
    
    // Reset to default
    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Butt });
    ops.push(Op::SetLineDashPattern {
        dash: LineDashPattern {
            offset: 0,
            dash_1: None,
            gap_1: None,
            dash_2: None,
            gap_2: None,
            dash_3: None,
            gap_3: None,
        },
    });
}

/// Render a double border side
fn render_double_border_side(
    ops: &mut Vec<Op>,
    side: BorderSide,
    bounds: &LogicalRect,
    widths: &BorderWidths,
    radii: &BorderRadii,
    page_height: f32,
    width: f32,
    margin_left: f32,
    margin_top: f32,
) {
    // Double border: render two lines with 1/3 width each, spaced by 1/3 width
    // The outer line is at the outer edge, the inner line is inset by 2/3 of the total width
    let line_width = width / 3.0;
    let inset = width * 2.0 / 3.0; // Distance from outer edge to inner line center
    
    ops.push(Op::SetOutlineThickness { pt: Pt(line_width) });
    
    // Render outer line (at the original bounds)
    render_solid_border_side(ops, side, bounds, widths, radii, page_height, margin_left, margin_top);
    
    // Create inset bounds for inner line
    // The inner line should be drawn on a rectangle that is inset by `inset` on all sides
    let inner_bounds = LogicalRect {
        origin: LogicalPosition {
            x: bounds.origin.x + inset,
            y: bounds.origin.y + inset,
        },
        size: LogicalSize {
            width: bounds.size.width - inset * 2.0,
            height: bounds.size.height - inset * 2.0,
        },
    };
    
    // Adjust widths for inner bounds (they should still render the same width)
    let inner_widths = BorderWidths {
        top: widths.top,
        right: widths.right,
        bottom: widths.bottom,
        left: widths.left,
    };
    
    // Render inner line
    render_solid_border_side(ops, side, &inner_bounds, &inner_widths, radii, page_height, margin_left, margin_top);
}

