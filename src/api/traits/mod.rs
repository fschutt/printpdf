//! Traits used in a PDF document
extern crate lopdf;

/// Object can be serialized to an `lopdf::Object`, such as a Dictionary, etc.
pub trait IntoPdfObject: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF object
    fn into_obj(self: Box<Self>)
    -> lopdf::Object;
}

/// Object can be used within a stream, such as a drawing operation, etc.
pub trait IntoPdfStreamOperation: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF stream operation
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>;
}
