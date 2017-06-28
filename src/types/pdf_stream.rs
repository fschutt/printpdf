//! A PDF stream consisting of various IntoPdfStream - able objects
//! **WARNING** A PDF stream MUST be referenced indirectly, while the
//! referencing dictionary must be referenced directly

extern crate lopdf;

use *;
use lopdf::{Stream, Dictionary};

#[derive(Debug)]
pub struct PdfStream {
    pub dictionary: Dictionary,
    objects: Vec<Box<IntoPdfStreamOperation>>
}

impl PdfStream {

    /// Creates a new PdfStream
    pub fn new()
    -> Self
    {
        Self {
            dictionary: Dictionary::new(),
            objects: Vec::new(),
        }
    }

    /// Adds a stream operation to the stream
    #[inline]
    pub fn add_operation(&mut self, operation: Box<IntoPdfStreamOperation>)
    {
        self.objects.place_back() <- operation;
    }

    /// Similar to the trait function, but only returns a single object
    pub fn into_obj(self) 
    -> lopdf::Stream {
        let mut stream_operations = Vec::<lopdf::content::Operation>::new();
        let dict = self.dictionary.clone();

        for object in self.objects.into_iter() {
          let mut object = object.into_stream_op().to_vec();
          stream_operations.append(&mut object);
        }

        let stream_content = lopdf::content::Content { operations: stream_operations };

        let mut stream = Stream::new(dict, stream_content.encode().unwrap());
        stream.compress();
        return stream
    }
}