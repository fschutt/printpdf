//! A PDF stream consisting of various IntoPdfStream - able objects
//! **WARNING** A PDF stream MUST be referenced indirectly, while the
//! referencing dictionary must be referenced directly

extern crate lopdf;

use *;
use lopdf::{Stream, Dictionary};

#[derive(Debug)]
pub struct PdfStream {
    pub dictionary: Dictionary,
    operations: Vec<lopdf::content::Operation>
}

impl PdfStream {

    /// Creates a new PdfStream
    pub fn new()
    -> Self
    {
        Self {
            dictionary: Dictionary::new(),
            operations: Vec::new(),
        }
    }

    /// Adds a number of operations to the stream
    #[inline]
    pub fn add_operations(&mut self, operation: Box<IntoPdfStreamOperation>)
    {
        for op in operation.into_stream_op() {
          self.operations.place_back() <-  op;
        }
    }

    /// Add one operation to the stream
    #[inline]
    pub fn add_operation<O>(&mut self, operation: O)
    -> () where O: Into<lopdf::content::Operation>
    {
        self.operations.place_back() <-  operation.into();
    }

    /// Similar to the trait function, but only returns a single object
    pub fn into_obj(self) 
    -> lopdf::Stream {
        let stream_content = lopdf::content::Content { operations: self.operations };
        let mut stream = Stream::new(self.dictionary, stream_content.encode().unwrap());
        // stream.compress();
        return stream
    }
}