//! Traits used in a PDF document
extern crate lopdf;

/// Object can be used within a stream, such as a drawing operation, etc.
pub trait IntoPdfStreamOperation: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF stream operation
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>;
}

// implement this trait for simple operations
#[cfg_attr(feature = "cargo-clippy", allow(boxed_local))]
impl IntoPdfStreamOperation for lopdf::content::Operation {
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        vec![*self]
    }
}
