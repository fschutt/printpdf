extern crate lopdf;

use traits::*;

#[derive(Debug)]
pub struct Svg {
    /* same as font: parse + store metrics, then in convert function, convert to pdf object */
    svg_data: Vec<u8>,
    width: u8,
    height: u8,
}

impl Svg {

    pub fn new<R>(svg_data: R)
    -> Self where R: ::std::io::Read
    {
        unimplemented!()
    }
}

impl IntoPdfObject for Svg {
    fn into(self)
    -> lopdf::Object
    {
        // make SVG to stream, then use it in the doument as a reference
        unimplemented!()
    }
} 