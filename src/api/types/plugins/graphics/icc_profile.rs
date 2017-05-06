//! ICC profile that can be embedded into a PDF

extern crate lopdf;

use *;

/// Type of the icc profile
#[derive(Debug, Clone)]
pub enum IccProfileType {
    Cmyk,
    Rgb,
    Grayscale,
}

/// Icc profile
#[derive(Debug, Clone)]
pub struct IccProfile {
    /// Binary Icc profile
    icc: Vec<u8>,
    /// CMYK or RGB or LAB icc profile?
    icc_type: IccProfileType, 
}

impl IccProfile {
    /// Creates a new Icc Profile
    pub fn new(icc: Vec<u8>, icc_type: IccProfileType)
    -> Self 
    {
        Self { icc, icc_type }
    }

    // todo: transfer functions, etc.
}

impl IntoPdfObject for IccProfile {
    fn into_obj(self: Box<Self>)
    -> lopdf::Object
    {
        use lopdf::{Dictionary as LoDictionary, 
                    Object as LoObject, 
                    Stream as LoStream};
        use lopdf::Object::*;
        use std::iter::FromIterator;

        let (num_icc_fields, alternate) = match self.icc_type {
            IccProfileType::Cmyk => (4, "DeviceCMYK"),
            IccProfileType::Rgb => (3, "DeviceRGB"),
            IccProfileType::Grayscale => (1, "DeviceGray"),
        };

        let stream = LoStream::new(LoDictionary::from_iter(vec![
                ("N", Integer(num_icc_fields)).into(),
                ("Alternate", Name(alternate.into())).into(),
                ("Length", Integer(self.icc.len() as i64).into())]),
            self.icc
        );

        Stream(stream)
    }
}