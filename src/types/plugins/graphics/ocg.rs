extern crate lopdf;

#[derive(Default, Debug)]
pub struct OCGList {
    /// The reference to the layer as well as a reference to the
    /// OCG dictionary
    pub(crate) layers: Vec<(OCGRef, lopdf::Object)>,
}

impl OCGList {
    /// Creates a new OCG list
    pub fn new()
    -> Self
    {
        Self::default()
    }

    /// Adds a new OCG List from a reference
    pub fn add_ocg(&mut self, obj: lopdf::Object)
    -> OCGRef
    {
        let len = self.layers.len();
        let ocg_ref = OCGRef::new(len);
        self.layers.push((ocg_ref.clone(), obj));
        ocg_ref
    }
}

impl Into<lopdf::Dictionary> for OCGList {
    #[cfg_attr(feature = "cargo-clippy", allow(needless_return))]
    fn into(self)
    -> lopdf::Dictionary
    {
        let mut dict = lopdf::Dictionary::new();

        for entry in self.layers {
            dict.set(entry.0.name, entry.1);
        }

        return dict;
    }
}

#[derive(Debug, Clone)]
pub struct OCGRef {
    /// The referenced name of the layer
    pub(crate) name: String,
}

impl OCGRef {
    /// Creates a new OCG reference from an index
    pub fn new(index: usize)
    -> Self
    {
        Self {
            name: format!("MC{}", index),
        }
    }
}
