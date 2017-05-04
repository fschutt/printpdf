//! Traits used in a PDF document
extern crate lopdf;

/// Object can be serialized to an `lopdf::Object`, such as a Dictionary, etc.
pub trait IntoPdfObject: ::std::fmt::Debug {
    fn into(self)
    -> lopdf::Object;
}

/// Object can be used within a stream, such as a drawing operation, etc.
pub trait IntoPdfStreamOperation
{
    fn into(self)
    -> lopdf::content::Operation;
}
