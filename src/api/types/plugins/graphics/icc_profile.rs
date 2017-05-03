//! Example implementation of an arbitrary PDF Blob
extern crate lopdf;

pub use traits::*;

#[derive(Debug, Clone)]
pub struct IccProfile {
    /// Binary Icc profile
    icc: Vec<u8>,
}

impl IntoPdfObject for IccProfile {
    fn into(self)
    -> lopdf::Object
    {
        // todo: contruct stream object, put the icc profile in it, etc.
        unimplemented!()
    }
}