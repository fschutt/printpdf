use printpdf::*;

const HTML_STRINGS: &[&str; 1] = &[
    // "<div style='background:red;padding:10px;'><div style='background:yellow;padding:20px;'></div></div>",
    "<p style='color:red;font-family:sans-serif'>Hello!</p><img src='dog-alpha.png' />",
];

pub struct ImgComponent { }

impl XmlComponentTrait for ImgComponent {

    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            accepts_text: false,
            args: vec![("src".to_string(), "String".to_string())]
        }
    }

    fn render_dom(
        &self,
        components: &XmlComponentMap,
        arguments: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        // TODO: parse image from arguments["src"]
        Ok(Dom::image(
            InternalImageRef::new_rawimage(
                translate_to_internal_rawimage(
                    &RawImage::decode_from_bytes(include_bytes!("./assets/img/dog_alpha.png")).unwrap()
                ) 
            ).unwrap()
        ).style(CssApiWrapper::empty()))
    }
}

fn main() -> Result<(), String> {

    for (i, h) in HTML_STRINGS.iter().enumerate() {

        let components = vec![XmlComponent {
            id: "img".to_string(),
            renderer: Box::new(ImgComponent { }),
            inherit_vars: false,
        }];

        let config = XmlRenderOptions {
            components,
            .. Default::default()
        };

        let doc = PdfDocument::new("HTML rendering demo")
            .with_html(h, config)?
            .save(&PdfSaveOptions::default());
        std::fs::write(format!("html{i}.pdf"), doc).unwrap();
    }

    Ok(())
}
