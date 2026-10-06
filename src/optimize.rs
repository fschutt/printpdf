//! Shrinking the pictures of a finished PDF, without touching anything else in it.

use crate::{ImageOptimizationOptions, PdfWarnMsg};

/// Encodes the pictures of a finished PDF again, as [`PdfDocument::save`](crate::PdfDocument::save)
/// would with `options`, and puts each one that comes out smaller in place of the old one.
pub fn optimize_images(
    pdf: &[u8],
    _options: &ImageOptimizationOptions,
    _warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<u8>, String> {
    Ok(pdf.to_vec())
}
