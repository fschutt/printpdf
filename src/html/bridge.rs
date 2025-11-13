//! Bridge module to translate azul-layout PDF operations to printpdf operations.
//!
//! This module converts the intermediate PDF representation from azul-layout
//! into printpdf's native Op enum, allowing us to leverage azul's layout engine
//! while using printpdf's PDF generation.

use azul_layout::pdf::{PdfColor, PdfOp as AzulPdfOp, PdfTextMatrix, TextItem as AzulTextItem};
use crate::{Color, Mm, Op, Pt, Rgb, TextItem as PrintpdfTextItem, TextMatrix as PrintpdfTextMatrix};

/// Convert an azul PdfColor to a printpdf Color
fn convert_color(color: &PdfColor) -> Color {
    match color {
        PdfColor::Rgb(c) | PdfColor::Rgba(c) => Color::Rgb(Rgb {
            r: c.r as f32 / 255.0,
            g: c.g as f32 / 255.0,
            b: c.b as f32 / 255.0,
            icc_profile: None,
        }),
        PdfColor::Cmyk { c, m, y, k } => Color::Cmyk(crate::Cmyk {
            c: *c,
            m: *m,
            y: *y,
            k: *k,
            icc_profile: None,
        }),
        PdfColor::Gray(g) => Color::Greyscale(crate::Greyscale {
            percent: *g,
            icc_profile: None,
        }),
    }
}

/// Convert an azul TextMatrix to a printpdf TextMatrix
fn convert_text_matrix(matrix: &PdfTextMatrix) -> PrintpdfTextMatrix {
    PrintpdfTextMatrix::Raw([
        matrix.a,
        matrix.b,
        matrix.c,
        matrix.d,
        matrix.e,
        matrix.f,
    ])
}

/// Convert azul TextItem to printpdf TextItem
fn convert_text_item(item: &AzulTextItem) -> PrintpdfTextItem {
    if item.adjustment == 0.0 {
        PrintpdfTextItem::Text(item.text.clone())
    } else {
        PrintpdfTextItem::Offset(item.adjustment)
    }
}

/// Convert a vector of azul PdfOps to printpdf Ops
///
/// # Arguments
/// * `azul_ops` - The azul PDF operations to convert
/// * `font_id_map` - A map from azul font IDs to printpdf font IDs (populated during conversion)
///
/// # Returns
/// A vector of printpdf Ops that can be added to a PDF page
pub fn convert_azul_ops_to_printpdf(
    azul_ops: &[AzulPdfOp],
    font_id_map: &mut std::collections::BTreeMap<String, crate::FontId>,
) -> Vec<Op> {
    let mut ops = Vec::new();

    for azul_op in azul_ops {
        match azul_op {
            AzulPdfOp::BeginPath => {
                // Path will be built up with subsequent ops
                // No direct equivalent, handled implicitly
            }

            AzulPdfOp::MoveTo { point } => {
                // Store for polygon construction
                // Will be handled when we encounter Fill/Stroke
            }

            AzulPdfOp::LineTo { point } => {
                // Store for polygon construction
            }

            AzulPdfOp::CurveTo { control1, control2, end } => {
                // Store for path construction
            }

            AzulPdfOp::ClosePath => {
                // Marks end of path
            }

            AzulPdfOp::Stroke => {
                // Will trigger polygon drawing
            }

            AzulPdfOp::Fill => {
                // Will trigger polygon drawing
            }

            AzulPdfOp::FillAndStroke => {
                // Will trigger polygon drawing with both fill and stroke
            }

            AzulPdfOp::SetStrokeColor { color } => {
                ops.push(Op::SetOutlineColor {
                    col: convert_color(color),
                });
            }

            AzulPdfOp::SetFillColor { color } => {
                ops.push(Op::SetFillColor {
                    col: convert_color(color),
                });
            }

            AzulPdfOp::SetLineWidth { width } => {
                ops.push(Op::SetOutlineThickness {
                    pt: Pt(*width),
                });
            }

            AzulPdfOp::SetLineDash { pattern, phase } => {
                ops.push(Op::SetLineDashPattern {
                    dash: crate::graphics::LineDashPattern {
                        dash_1: pattern.get(0).copied().map(|v| v as i64),
                        gap_1: pattern.get(1).copied().map(|v| v as i64),
                        dash_2: pattern.get(2).copied().map(|v| v as i64),
                        gap_2: pattern.get(3).copied().map(|v| v as i64),
                        dash_3: pattern.get(4).copied().map(|v| v as i64),
                        gap_3: pattern.get(5).copied().map(|v| v as i64),
                        offset: *phase as i64,
                    },
                });
            }

            AzulPdfOp::BeginText => {
                ops.push(Op::StartTextSection);
            }

            AzulPdfOp::EndText => {
                ops.push(Op::EndTextSection);
            }

            AzulPdfOp::SetTextFont { font_id, size } => {
                // Map azul font ID to printpdf font ID
                let printpdf_font_id = font_id_map
                    .entry(font_id.0.clone())
                    .or_insert_with(|| crate::FontId(font_id.0.clone()))
                    .clone();

                ops.push(Op::SetFontSize {
                    size: Pt(*size),
                    font: printpdf_font_id,
                });
            }

            AzulPdfOp::SetTextMatrix { matrix } => {
                ops.push(Op::SetTextMatrix {
                    matrix: convert_text_matrix(matrix),
                });
            }

            AzulPdfOp::ShowText { text } => {
                // This shouldn't be used with our implementation
                // We use ShowPositionedText instead
            }

            AzulPdfOp::ShowPositionedText { items } => {
                let printpdf_items: Vec<PrintpdfTextItem> = items
                    .iter()
                    .map(convert_text_item)
                    .collect();

                // We need to know which font to use - this should be set by SetTextFont
                // For now, we'll need to track the current font
                // This is a simplification - in reality we'd need state tracking
            }

            AzulPdfOp::DrawImage { xobject_id, rect } => {
                // Image drawing
            }

            AzulPdfOp::SaveState => {
                ops.push(Op::SaveGraphicsState);
            }

            AzulPdfOp::RestoreState => {
                ops.push(Op::RestoreGraphicsState);
            }

            AzulPdfOp::Transform { matrix } => {
                // Transform the coordinate system
            }

            AzulPdfOp::ClipRect { rect } => {
                // Set clipping region
            }
        }
    }

    ops
}

/// Stateful converter that tracks current path, font, etc.
pub struct AzulToPrintpdfConverter {
    current_path: Vec<(f32, f32)>,
    current_font: Option<crate::FontId>,
    font_id_map: std::collections::BTreeMap<String, crate::FontId>,
}

impl AzulToPrintpdfConverter {
    pub fn new() -> Self {
        Self {
            current_path: Vec::new(),
            current_font: None,
            font_id_map: std::collections::BTreeMap::new(),
        }
    }

    pub fn convert(&mut self, azul_ops: &[AzulPdfOp]) -> Vec<Op> {
        let mut ops = Vec::new();

        for azul_op in azul_ops {
            self.convert_single_op(azul_op, &mut ops);
        }

        ops
    }

    fn convert_single_op(&mut self, azul_op: &AzulPdfOp, ops: &mut Vec<Op>) {
        match azul_op {
            AzulPdfOp::BeginPath => {
                self.current_path.clear();
            }

            AzulPdfOp::MoveTo { point } => {
                self.current_path.push((point.x, point.y));
            }

            AzulPdfOp::LineTo { point } => {
                self.current_path.push((point.x, point.y));
            }

            AzulPdfOp::ClosePath => {
                // Path is closed
            }

            AzulPdfOp::Fill => {
                if self.current_path.len() >= 3 {
                    let polygon = crate::graphics::Polygon {
                        rings: vec![crate::graphics::PolygonRing {
                            points: self.current_path
                                .iter()
                                .map(|(x, y)| crate::graphics::LinePoint {
                                    p: crate::graphics::Point::new(Mm(*x * 0.3527777778), Mm(*y * 0.3527777778)),
                                    bezier: false,
                                })
                                .collect(),
                        }],
                        mode: crate::graphics::PaintMode::Fill,
                        winding_order: crate::graphics::WindingOrder::NonZero,
                    };
                    ops.push(Op::DrawPolygon { polygon });
                }
                self.current_path.clear();
            }

            AzulPdfOp::Stroke => {
                if self.current_path.len() >= 2 {
                    let line = crate::graphics::Line {
                        points: self.current_path
                            .iter()
                            .map(|(x, y)| crate::graphics::LinePoint {
                                p: crate::graphics::Point::new(Mm(*x * 0.3527777778), Mm(*y * 0.3527777778)),
                                bezier: false,
                            })
                            .collect(),
                        is_closed: false,
                    };
                    ops.push(Op::DrawLine { line });
                }
                self.current_path.clear();
            }

            AzulPdfOp::SetStrokeColor { color } => {
                ops.push(Op::SetOutlineColor {
                    col: convert_color(color),
                });
            }

            AzulPdfOp::SetFillColor { color } => {
                ops.push(Op::SetFillColor {
                    col: convert_color(color),
                });
            }

            AzulPdfOp::SetLineWidth { width } => {
                ops.push(Op::SetOutlineThickness {
                    pt: Pt(*width),
                });
            }

            AzulPdfOp::BeginText => {
                ops.push(Op::StartTextSection);
            }

            AzulPdfOp::EndText => {
                ops.push(Op::EndTextSection);
            }

            AzulPdfOp::SetTextFont { font_id, size } => {
                let printpdf_font_id = self.font_id_map
                    .entry(font_id.0.clone())
                    .or_insert_with(|| crate::FontId(font_id.0.clone()))
                    .clone();

                self.current_font = Some(printpdf_font_id.clone());

                ops.push(Op::SetFontSize {
                    size: Pt(*size),
                    font: printpdf_font_id,
                });
            }

            AzulPdfOp::SetTextMatrix { matrix } => {
                ops.push(Op::SetTextMatrix {
                    matrix: convert_text_matrix(matrix),
                });
            }

            AzulPdfOp::ShowPositionedText { items } => {
                if let Some(font) = &self.current_font {
                    let printpdf_items: Vec<PrintpdfTextItem> = items
                        .iter()
                        .map(convert_text_item)
                        .collect();

                    ops.push(Op::WriteText {
                        items: printpdf_items,
                        font: font.clone(),
                    });
                }
            }

            AzulPdfOp::SaveState => {
                ops.push(Op::SaveGraphicsState);
            }

            AzulPdfOp::RestoreState => {
                ops.push(Op::RestoreGraphicsState);
            }

            _ => {
                // Other operations not yet fully implemented
            }
        }
    }

    pub fn get_font_id_map(&self) -> &std::collections::BTreeMap<String, crate::FontId> {
        &self.font_id_map
    }
}
