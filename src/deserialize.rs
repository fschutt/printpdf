use crate::PdfDocument;

pub fn parse_pdf_from_bytes(bytes: &[u8]) -> Result<PdfDocument, String> {
    Ok(PdfDocument::new("parsed"))
}
