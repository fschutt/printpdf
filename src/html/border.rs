//! Border and border-radius rendering utilities for PDF generation.
//!
//! This module provides comprehensive border rendering including:
//! - All four sides (top, right, bottom, left) rendered individually
//! - Border-radius support with proper corner curves
//! - Border styles (solid, dashed, dotted, etc.)
//! - Reusable path generation for clipping rectangles

use azul_core::geom::LogicalRect;
use azul_css::props::basic::{pixel::DEFAULT_FONT_SIZE, ColorU};
use azul_layout::solver3::display_list::{StyleBorderColors, StyleBorderStyles, StyleBorderWidths};
use azul_css::props::style::border_radius::StyleBorderRadius;
use crate::{Color, Mm, Op, Pt, Rgb, LineCapStyle, LineDashPattern};

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

/// Render all four borders with full support for border-radius and styles
/// 
/// NOTE: This function expects bounds in CSS coordinates (top-left origin).
/// The Y-coordinate conversion to PDF space (bottom-left origin) happens internally.
pub fn render_border(ops: &mut Vec<Op>, config: &BorderConfig) {
    let widths = &config.widths;
    let colors = &config.colors;
    let styles = &config.styles;
    let radii = &config.radii;
    let bounds = &config.bounds;

    // Check if all sides are identical (common case optimization)
    let all_same = widths.top == widths.right && widths.top == widths.bottom && widths.top == widths.left
        && colors.top == colors.right && colors.top == colors.bottom && colors.top == colors.left
        && styles.top == styles.right && styles.top == styles.bottom && styles.top == styles.left
        && styles.top == BorderStyleType::Solid
        && radii.top_left == (0.0, 0.0) && radii.top_right == (0.0, 0.0) 
        && radii.bottom_right == (0.0, 0.0) && radii.bottom_left == (0.0, 0.0);

    if all_same && widths.top > 0.0 && colors.top.a > 0 {
        // Optimized path: render as a single stroked rectangle
        render_unified_border(ops, bounds, widths.top, colors.top, config.page_height);
        return;
    }

    // Render each side individually
    if widths.top > 0.0 && styles.top != BorderStyleType::None && colors.top.a > 0 {
        render_border_side(ops, BorderSide::Top, bounds, widths, colors, styles, radii, config.page_height);
    }
    if widths.right > 0.0 && styles.right != BorderStyleType::None && colors.right.a > 0 {
        render_border_side(ops, BorderSide::Right, bounds, widths, colors, styles, radii, config.page_height);
    }
    if widths.bottom > 0.0 && styles.bottom != BorderStyleType::None && colors.bottom.a > 0 {
        render_border_side(ops, BorderSide::Bottom, bounds, widths, colors, styles, radii, config.page_height);
    }
    if widths.left > 0.0 && styles.left != BorderStyleType::None && colors.left.a > 0 {
        render_border_side(ops, BorderSide::Left, bounds, widths, colors, styles, radii, config.page_height);
    }
}

/// Render a border as a single stroked rectangle (optimization for uniform borders)
fn render_unified_border(
    ops: &mut Vec<Op>,
    bounds: &LogicalRect,
    width: f32,
    color: ColorU,
    page_height: f32,
) {
    ops.push(Op::SaveGraphicsState);
    
    // Convert to PDF coordinate space (bottom-left origin)
    let y = page_height - bounds.origin.y - bounds.size.height;
    
    // Adjust for stroke centering: the stroke is centered on the path,
    // so we need to inset by half the width to keep the border inside
    let half_width = width / 2.0;
    let x = bounds.origin.x + half_width;
    let y_adj = y + half_width;
    let w = bounds.size.width - width;
    let h = bounds.size.height - width;
    
    // Create a closed rectangle path
    let polygon = crate::graphics::Polygon {
        rings: vec![crate::graphics::PolygonRing {
            points: vec![
                crate::graphics::LinePoint {
                    p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm(y_adj * 0.3527777778)),
                    bezier: false,
                },
                crate::graphics::LinePoint {
                    p: crate::graphics::Point::new(Mm((x + w) * 0.3527777778), Mm(y_adj * 0.3527777778)),
                    bezier: false,
                },
                crate::graphics::LinePoint {
                    p: crate::graphics::Point::new(Mm((x + w) * 0.3527777778), Mm((y_adj + h) * 0.3527777778)),
                    bezier: false,
                },
                crate::graphics::LinePoint {
                    p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y_adj + h) * 0.3527777778)),
                    bezier: false,
                },
            ],
        }],
        mode: crate::graphics::PaintMode::Stroke,
        winding_order: crate::graphics::WindingOrder::NonZero,
    };
    
    ops.push(Op::SetOutlineColor { col: convert_color(&color) });
    ops.push(Op::SetOutlineThickness { pt: Pt(width) });
    ops.push(Op::DrawPolygon { polygon });
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
) {
    let (width, color, style) = match side {
        BorderSide::Top => (widths.top, colors.top, styles.top),
        BorderSide::Right => (widths.right, colors.right, styles.right),
        BorderSide::Bottom => (widths.bottom, colors.bottom, styles.bottom),
        BorderSide::Left => (widths.left, colors.left, styles.left),
    };

    ops.push(Op::SaveGraphicsState);

    // Set color and width
    ops.push(Op::SetOutlineColor { col: convert_color(&color) });
    ops.push(Op::SetOutlineThickness { pt: Pt(width) });

    // Handle different border styles
    match style {
        BorderStyleType::Solid => {
            render_solid_border_side(ops, side, bounds, widths, radii, page_height);
        }
        BorderStyleType::Dashed => {
            render_dashed_border_side(ops, side, bounds, widths, radii, page_height, width);
        }
        BorderStyleType::Dotted => {
            render_dotted_border_side(ops, side, bounds, widths, radii, page_height, width);
        }
        BorderStyleType::Double => {
            render_double_border_side(ops, side, bounds, widths, radii, page_height, width);
        }
        _ => {
            // For groove, ridge, inset, outset - fall back to solid for now
            render_solid_border_side(ops, side, bounds, widths, radii, page_height);
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
) {
    let x = bounds.origin.x;
    let w = bounds.size.width;
    let h = bounds.size.height;
    
    // Convert CSS Y (top-left origin) to PDF Y (bottom-left origin)
    let pdf_y_bottom = page_height - bounds.origin.y - bounds.size.height;
    let pdf_y_top = page_height - bounds.origin.y;
    
    let line = match side {
        BorderSide::Top => {
            // CSS top border is at Y=bounds.origin.y, which is pdf_y_top in PDF space
            let start_x = x + radii.top_left.0.max(widths.left / 2.0);
            let end_x = x + w - radii.top_right.0.max(widths.right / 2.0);
            
            create_line_with_corners(
                start_x, pdf_y_top, end_x, pdf_y_top,
                radii.top_left, radii.top_right,
                Corner::TopLeft, Corner::TopRight,
                widths.left, widths.right, widths.top,
            )
        }
        BorderSide::Right => {
            // Right border: from top to bottom
            let start_y = pdf_y_top - radii.top_right.1.max(widths.top / 2.0);
            let end_y = pdf_y_bottom + radii.bottom_right.1.max(widths.bottom / 2.0);
            
            create_line_with_corners(
                x + w, start_y, x + w, end_y,
                radii.top_right, radii.bottom_right,
                Corner::TopRight, Corner::BottomRight,
                widths.top, widths.bottom, widths.right,
            )
        }
        BorderSide::Bottom => {
            // CSS bottom border is at Y=bounds.origin.y+height, which is pdf_y_bottom in PDF space
            let start_x = x + w - radii.bottom_right.0.max(widths.right / 2.0);
            let end_x = x + radii.bottom_left.0.max(widths.left / 2.0);
            
            create_line_with_corners(
                start_x, pdf_y_bottom, end_x, pdf_y_bottom,
                radii.bottom_right, radii.bottom_left,
                Corner::BottomRight, Corner::BottomLeft,
                widths.right, widths.left, widths.bottom,
            )
        }
        BorderSide::Left => {
            // Left border: from bottom to top
            let start_y = pdf_y_bottom + radii.bottom_left.1.max(widths.bottom / 2.0);
            let end_y = pdf_y_top - radii.top_left.1.max(widths.top / 2.0);
            
            create_line_with_corners(
                x, start_y, x, end_y,
                radii.bottom_left, radii.top_left,
                Corner::BottomLeft, Corner::TopLeft,
                widths.bottom, widths.top, widths.left,
            )
        }
    };

    ops.push(Op::DrawLine { line });
}

/// Create a line with optional curved corners
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
) -> crate::graphics::Line {
    let mut points = Vec::new();

    // Add start point (with optional curve)
    if start_radius.0 > 0.1 || start_radius.1 > 0.1 {
        // Add bezier curve for start corner
        add_corner_curve(&mut points, start_x, start_y, start_radius, start_corner, start_perpendicular_width, parallel_width, true);
    } else {
        points.push(crate::graphics::LinePoint {
            p: crate::graphics::Point::new(Mm(start_x * 0.3527777778), Mm(start_y * 0.3527777778)),
            bezier: false,
        });
    }

    // Add end point (with optional curve)
    if end_radius.0 > 0.1 || end_radius.1 > 0.1 {
        // Add bezier curve for end corner
        add_corner_curve(&mut points, end_x, end_y, end_radius, end_corner, end_perpendicular_width, parallel_width, false);
    } else {
        points.push(crate::graphics::LinePoint {
            p: crate::graphics::Point::new(Mm(end_x * 0.3527777778), Mm(end_y * 0.3527777778)),
            bezier: false,
        });
    }

    crate::graphics::Line {
        points,
        is_closed: false,
    }
}

/// Add a corner curve to the line points
fn add_corner_curve(
    points: &mut Vec<crate::graphics::LinePoint>,
    x: f32, y: f32,
    radius: (f32, f32),
    corner: Corner,
    _perpendicular_width: f32,
    _parallel_width: f32,
    _is_start: bool,
) {
    // Simplified: just add the point without curve for now
    // Full implementation would add proper bezier control points
    points.push(crate::graphics::LinePoint {
        p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm(y * 0.3527777778)),
        bezier: false,
    });
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
    
    render_solid_border_side(ops, side, bounds, widths, radii, page_height);
    
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
    
    render_solid_border_side(ops, side, bounds, widths, radii, page_height);
    
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
) {
    // Double border: render two lines with 1/3 width each, spaced by 1/3 width
    let line_width = width / 3.0;
    let gap = width / 3.0;
    
    ops.push(Op::SetOutlineThickness { pt: Pt(line_width) });
    
    // Render outer line
    render_solid_border_side(ops, side, bounds, widths, radii, page_height);
    
    // Render inner line (offset by line_width + gap)
    let mut inner_bounds = *bounds;
    let offset = line_width + gap;
    
    match side {
        BorderSide::Top => inner_bounds.origin.y += offset,
        BorderSide::Right => inner_bounds.origin.x += inner_bounds.size.width - offset,
        BorderSide::Bottom => inner_bounds.origin.y += inner_bounds.size.height - offset,
        BorderSide::Left => inner_bounds.origin.x += offset,
    }
    
    render_solid_border_side(ops, side, &inner_bounds, widths, radii, page_height);
}

/// Generate a clipping path with border-radius
/// This can be used to clip content to rounded rectangles
pub fn create_clip_path(bounds: &LogicalRect, radii: &BorderRadii, page_height: f32) -> crate::graphics::Line {
    let x = bounds.origin.x;
    let y = page_height - bounds.origin.y - bounds.size.height;
    let w = bounds.size.width;
    let h = bounds.size.height;

    let mut points = Vec::new();

    // Top-left corner
    if radii.top_left.0 > 0.1 || radii.top_left.1 > 0.1 {
        add_rounded_corner(&mut points, x, y, radii.top_left, Corner::TopLeft);
    } else {
        points.push(crate::graphics::LinePoint {
            p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm(y * 0.3527777778)),
            bezier: false,
        });
    }

    // Top-right corner
    if radii.top_right.0 > 0.1 || radii.top_right.1 > 0.1 {
        add_rounded_corner(&mut points, x + w, y, radii.top_right, Corner::TopRight);
    } else {
        points.push(crate::graphics::LinePoint {
            p: crate::graphics::Point::new(Mm((x + w) * 0.3527777778), Mm(y * 0.3527777778)),
            bezier: false,
        });
    }

    // Bottom-right corner
    if radii.bottom_right.0 > 0.1 || radii.bottom_right.1 > 0.1 {
        add_rounded_corner(&mut points, x + w, y + h, radii.bottom_right, Corner::BottomRight);
    } else {
        points.push(crate::graphics::LinePoint {
            p: crate::graphics::Point::new(Mm((x + w) * 0.3527777778), Mm((y + h) * 0.3527777778)),
            bezier: false,
        });
    }

    // Bottom-left corner
    if radii.bottom_left.0 > 0.1 || radii.bottom_left.1 > 0.1 {
        add_rounded_corner(&mut points, x, y + h, radii.bottom_left, Corner::BottomLeft);
    } else {
        points.push(crate::graphics::LinePoint {
            p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y + h) * 0.3527777778)),
            bezier: false,
        });
    }

    crate::graphics::Line {
        points,
        is_closed: true,
    }
}

/// Add a rounded corner with bezier curve
fn add_rounded_corner(
    points: &mut Vec<crate::graphics::LinePoint>,
    x: f32, y: f32,
    radius: (f32, f32),
    corner: Corner,
) {
    // Bezier control point offset (magic number for circular arcs: 4/3 * tan(π/8) ≈ 0.5522847498)
    const KAPPA: f32 = 0.5522847498;
    
    let (rx, ry) = radius;
    let cx = rx * KAPPA;
    let cy = ry * KAPPA;

    match corner {
        Corner::TopLeft => {
            // Start from left side, curve to top
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y + ry) * 0.3527777778)),
                bezier: false,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y + ry - cy) * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x + rx - cx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x + rx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: false,
            });
        }
        Corner::TopRight => {
            // Start from top, curve to right
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x - rx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: false,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x - rx + cx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y + ry - cy) * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y + ry) * 0.3527777778)),
                bezier: false,
            });
        }
        Corner::BottomRight => {
            // Start from right, curve to bottom
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y - ry) * 0.3527777778)),
                bezier: false,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y - ry + cy) * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x - rx + cx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x - rx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: false,
            });
        }
        Corner::BottomLeft => {
            // Start from bottom, curve to left
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x + rx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: false,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm((x + rx - cx) * 0.3527777778), Mm(y * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y - ry + cy) * 0.3527777778)),
                bezier: true,
            });
            points.push(crate::graphics::LinePoint {
                p: crate::graphics::Point::new(Mm(x * 0.3527777778), Mm((y - ry) * 0.3527777778)),
                bezier: false,
            });
        }
    }
}
