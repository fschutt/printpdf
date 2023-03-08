//! ICC profile that can be embedded into a PDF

use lopdf;

/// Type of the icc profile
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum IccProfileType {
    Cmyk,
    Rgb,
    Greyscale,
}

/// Icc profile
#[derive(Debug, Clone, PartialEq)]
pub struct IccProfile {
    /// Binary Icc profile
    icc: Vec<u8>,
    /// CMYK or RGB or LAB icc profile?
    icc_type: IccProfileType,
    /// Does the ICC profile have an "Alternate" version or not?
    pub has_alternate: bool,
    /// Does the ICC profile have an "Range" dictionary
    /// Really not sure why this is needed, but this is needed on the documents Info dictionary
    pub has_range: bool,
}

impl IccProfile {
    /// Creates a new Icc Profile
    pub fn new(icc: Vec<u8>, icc_type: IccProfileType) -> Self {
        Self {
            icc,
            icc_type,
            has_alternate: true,
            has_range: false,
        }
    }

    /// Does the ICC profile have an alternate version (such as "DeviceCMYk")?
    #[inline]
    pub fn with_alternate_profile(mut self, has_alternate: bool) -> Self {
        self.has_alternate = has_alternate;
        self
    }

    /// Does the ICC profile have an "Range" dictionary?
    #[inline]
    pub fn with_range(mut self, has_range: bool) -> Self {
        self.has_range = has_range;
        self
    }
}

impl From<IccProfile> for lopdf::Stream {
    fn from(val: IccProfile) -> Self {
        use lopdf::Object::*;
        use lopdf::{Dictionary as LoDictionary, Stream as LoStream};
        use std::iter::FromIterator;

        let (num_icc_fields, alternate) = match val.icc_type {
            IccProfileType::Cmyk => (4, "DeviceCMYK"),
            IccProfileType::Rgb => (3, "DeviceRGB"),
            IccProfileType::Greyscale => (1, "DeviceGray"),
        };

        let mut stream_dict = LoDictionary::from_iter(vec![
            ("N", Integer(num_icc_fields)),
            ("Length", Integer(val.icc.len() as i64)),
        ]);

        if val.has_alternate {
            stream_dict.set("Alternate", Name(alternate.into()));
        }

        if val.has_range {
            stream_dict.set(
                "Range",
                Array(vec![
                    Real(0.0),
                    Real(1.0),
                    Real(0.0),
                    Real(1.0),
                    Real(0.0),
                    Real(1.0),
                    Real(0.0),
                    Real(1.0),
                ]),
            );
        }

        LoStream::new(stream_dict, val.icc)
    }
}

/// Named reference for an ICC profile
#[derive(Debug, Clone, PartialEq)]
pub struct IccProfileRef {
    pub(crate) name: String,
}

impl IccProfileRef {
    /// Creates a new IccProfileRef
    pub fn new(index: usize) -> Self {
        Self {
            name: format!("/ICC{index}"),
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct IccProfileList {
    profiles: Vec<IccProfile>,
}

impl IccProfileList {
    /// Creates a new IccProfileList
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an ICC profile
    pub fn add_profile(&mut self, profile: IccProfile) -> IccProfileRef {
        let cur_len = self.profiles.len();
        self.profiles.push(profile);
        IccProfileRef::new(cur_len)
    }
}
