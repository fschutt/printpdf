//! Out of bounds errors
extern crate error_chain;

use super::super::api;

error_chain! {

    types {
        Error, ErrorKind, ResultExt, Result;
    }

    errors {
       PdfPageIndexError {
           description("Page index out of bounds")
           display("Page index out of bounds")
       }
       PdfLayerIndexError {
           description("PDF layer index out of bounds")
           display("PDF layer index out of bounds")
       }
       PdfMarkerIndexError {
           description("Page marker index out of bounds")
           display("Page marker index out of bounds")
       }
    }
}

