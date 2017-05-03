//! A PDF stream consisting of various IntoPdfStream - able objects
extern crate lopdf;

pub struct PdfStream {
    objects: Vec<Box<IntoPdfStreamOperation>>
}

impl IntoPdfObject for PdfStream {

    fn into(self) -> lopdf::Object {
        let stream_contents = self.objects
                                  .into_iter()
                                  .map(|obj| { 
                                      object.into::<lopdf::content::Operation>() 
                                  }).collect();
        let stream = lopdf::Stream { operations: stream_contents };
        lopdf::Object::Stream(stream.encode())
    }
}