//! Traits used in a PDF document
extern crate lopdf_bugfix_19072017 as lopdf;

/// Object can be serialized to an `lopdf::Object`, such as a Dictionary, etc.
pub trait IntoPdfObject: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF object
    fn into_obj(self: Box<Self>)
    -> Vec<lopdf::Object>;
}


// implement this trait for simple operations
impl IntoPdfObject for lopdf::Object {
    fn into_obj(self: Box<Self>)
    -> Vec<lopdf::Object>
    {
        vec![*self]
    }
}

/// Object can be used within a stream, such as a drawing operation, etc.
pub trait IntoPdfStreamOperation: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF stream operation
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>;
}

// implement this trait for simple operations
impl IntoPdfStreamOperation for lopdf::content::Operation {
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        vec![*self]
    }
}
