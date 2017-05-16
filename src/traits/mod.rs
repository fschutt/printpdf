//! Traits used in a PDF document
extern crate lopdf;
extern crate chrono;

use std::sync::{Arc, Mutex};
use indices::*;
use *;
use std::io::prelude::*;

/// Object can be serialized to an `lopdf::Object`, such as a Dictionary, etc.
pub trait IntoPdfObject: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF object
    fn into_obj(self: Box<Self>)
    -> Vec<lopdf::Object>;
}

/// Object can be used within a stream, such as a drawing operation, etc.
pub trait IntoPdfStreamOperation: ::std::fmt::Debug {
    /// Consumes the object and converts it to an PDF stream operation
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>;
}

impl IntoPdfStreamOperation for lopdf::content::Operation {
    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        vec![*self]
    }
}

/*
/// Extension trait for Arc<Mutex<PDFDocument>>
pub trait PdfDocumentFunction {    
    fn check_for_errors(&self) -> std::result::Result<(), Error>;
    fn repair_errors(&self, conformance: PdfConformance) -> std::result::Result<(), Error>;
    fn with_trapping(self, trapping: bool) -> Self;
    fn with_document_id(self, id: String) -> Self;
    fn with_document_version(self, version: u32) -> Self;
    fn with_conformance(self, conformance: PdfConformance) -> Self;
    fn with_title<S>(self, new_title: S) where S: Into<String>;
    fn with_mod_date(self, mod_date: chrono::DateTime<chrono::Local>) -> Self;
    fn add_page<S>(&self, x_mm: f64, y_mm: f64, inital_layer_name: S) -> (PdfPageIndex, PdfLayerIndex) where S: Into<String>;
    fn add_arbitrary_content<C>(&self, content: Box<C>) -> PdfContentIndex where C: 'static + IntoPdfObject;
    fn add_font<R>(&self, font_stream: R) -> std::result::Result<FontIndex, Error> where R: Read;
    fn add_svg<R>(&self, svg_data: R) -> SvgIndex where R: Read;
    fn get_page(&self, page: PdfPageIndex) -> &mut PdfPageReference;
    unsafe fn get_inner(self) -> (lopdf::Document, Vec<lopdf::Object>);
    fn save<W: Write + Seek>(self, target: &mut W) -> std::result::Result<(), Error>;
}
*/

