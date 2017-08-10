//! Errors related to PDF content
error_chain! {

    errors {
        FontFaceError {
           description("Invalid or corrupt font face")
           display("Invalid or corrupt font face")
        }
    }
}
