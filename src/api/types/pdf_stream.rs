//! A PDF stream consisting of various IntoPdfStream - able objects

extern crate lopdf;

use traits::*;
use lopdf::{Stream, Dictionary};

#[derive(Debug)]
pub struct PdfStream {
    dictionary: Dictionary,
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
}

impl IntoPdfObject for PdfStream {

    fn into_obj(self: Box<Self>) -> lopdf::Object {
        
        let mut stream_operations = Vec::<lopdf::content::Operation>::new();
        let dict = self.dictionary.clone();

        for object in self.objects.into_iter() {
          let mut object = object.into_stream_op().to_vec();
          stream_operations.append(&mut object);
        }

        let stream_content = lopdf::content::Content { operations: stream_operations };

        let mut stream = Stream::new(dict, stream_content.encode().unwrap());
        stream.compress();
        lopdf::Object::Stream(stream)
    }
}