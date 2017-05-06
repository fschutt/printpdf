//! Stub plugin for XMP Metadata streams, to be expanded later

/// Initial struct for Xmp metatdata. This should be expanded later for XML handling, etc.
/// Right now it just fills out the necessary fields
#[derive(Debug)]
pub struct XmpMetadata {
    /// "default" or to be left empty. Usually "default".
    rendition_class: Option<String>,
}