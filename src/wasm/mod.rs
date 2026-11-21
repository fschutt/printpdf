//! A JSON API for calling printpdf from JavaScript.
//!
//! JS code cannot share Rust structs, so every function takes one JSON string and returns one
//! JSON string. [`api`] holds those functions (the `Pdf_*` family, exported with `wasm-bindgen`
//! on wasm targets); [`structs`] holds the input and output types and the Rust function each
//! one calls, which work on every target without any JSON.
//!
//! The functions cover the common round trips: HTML to a document, PDF bytes to a document, a
//! document to PDF bytes, a page to SVG, the resources of a page, registering fonts once, and
//! decoding an image. Documents travel as the JSON form of [`crate::PdfDocument`].

pub mod api;
pub mod structs;
