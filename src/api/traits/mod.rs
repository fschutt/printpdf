//! Traits used in a PDF document
extern crate lopdf;


pub trait IntoPdfObject {
    fn into(self)
    -> lopdf::Object;
}

pub trait IntoPdfStreamOperation
{
    fn into(self)
    -> lopdf::content::Operation;
}
