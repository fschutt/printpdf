//! Errors related to PDF content
extern crate error_chain;

error_chain! {

    errors {
        FontFaceError {
           description("Invalid or corrupt font face")
           display("Invalid or corrupt font face")
        }
    }
}