//! Embedding fonts in 2D for Pdf
extern crate lopdf;

use traits::*;
use errors::*;

#[derive(Debug, Clone)]
pub struct Font {
    font_bytes: Vec<u8>
}

impl Font {
    pub fn new<R>(font_stream: R)
    -> ::std::result::Result<Self, Error> where R: ::std::io::Read
    {
        // read font from stream and parse font metrics
        unimplemented!()
    }
}

impl IntoPdfObject for Font {
    fn into(self)
    -> lopdf::Object
    {
        // todo: make stream from font, embed stream
        unimplemented!()
    }
}