//! ICC profile that can be embedded into a PDF

extern crate lopdf;

use *;

#[derive(Debug, Clone)]
pub struct IccProfile {
    /// Binary Icc profile
    icc: Vec<u8>,
}

impl IntoPdfObject for IccProfile {
    fn into_obj(self: Box<Self>)
    -> lopdf::Object
    {
        // todo: contruct stream object, put the icc profile in it, etc.
        unimplemented!()
    }
}