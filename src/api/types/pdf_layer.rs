//! One PDF layer = one optional content group

use super::*;
use errors::*;

/// One layer of PDF data
#[derive(Debug, Clone)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the OCG
    name: String,
}

impl PdfLayer {
    
    /// Create a new layer
    #[inline]
    pub fn new<S>(name: S)
    -> Self where S: Into<String>
    {
        Self {
            name: name.into(),
        }
    }
}