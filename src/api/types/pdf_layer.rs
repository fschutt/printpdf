//! One PDF layer = one optional content group

use super::*;
use errors::*;

/// One layer of PDF data
#[derive(Debug, Clone)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the OCG
    name: String,
    /// Markers on this layer
    markers: Vec<PdfMarker>,
}

impl PdfLayer {
    
    /// Create a new layer
    #[inline]
    pub fn new<S>(name: S)
    -> Self where S: Into<String>
    {
        Self{
            name: name.into(),
            markers: Vec::new(),
        }
    }

    /// Add a marker to the layer
    #[inline]
    pub fn add_marker(&mut self, x_mm: f32, y_mm: f32)
    -> usize
    {
        self.markers.push(PdfMarker::new(x_mm, y_mm));
        self.markers.len() - 1
    }

    /// Get a reference to a marker
    pub fn get_marker(&self, index: &usize)
    -> ::std::result::Result<&PdfMarker, Error>
    {
        use errors::index_error::ErrorKind::*;
        self.markers.get(*index)
                    .ok_or(Error::from_kind(IndexError(PdfMarkerIndexError)))
    }

    /// Get a mutable reference to a marker
    pub fn get_mut_marker(&mut self, index: &usize)
    -> ::std::result::Result<&mut PdfMarker, Error>
    {
        use errors::index_error::ErrorKind::*;
        self.markers.get_mut(*index)
                    .ok_or(Error::from_kind(IndexError(PdfMarkerIndexError)))
    }
}