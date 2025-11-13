use azul_core::{
    dom::{Dom, NodeData, NodeType},
    styled_dom::StyledDom,
    xml::{
        normalize_casing, CompileError, ComponentArguments,
        FilteredComponentArguments, RenderDomError, XmlComponent, XmlComponentMap,
        XmlComponentTrait, XmlNode, XmlTextContent,
    },
};

/// Macro to generate HTML element components with default CSS
/// Each HTML tag becomes a component that renders the corresponding DOM node
macro_rules! html_component {
    ($name:ident, $tag:expr, $node_type:expr) => {
        html_component!($name, $tag, $node_type, "");
    };
    ($name:ident, $tag:expr, $node_type:expr, $default_css:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            node: XmlNode,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    node: XmlNode::new($tag),
                }
            }
        }

        impl XmlComponentTrait for $name {
            fn get_available_arguments(&self) -> ComponentArguments {
                ComponentArguments::default()
            }

            fn render_dom(
                &self,
                _: &XmlComponentMap,
                _: &FilteredComponentArguments,
                text: &XmlTextContent,
            ) -> Result<StyledDom, RenderDomError> {
                // Create the DOM node manually
                let node_data = NodeData::new($node_type);
                let mut dom = Dom {
                    root: node_data,
                    children: Vec::new().into(),
                    estimated_total_children: 0,
                };
                
                // Add text content if present
                if let Some(text_str) = text.as_ref() {
                    if !text_str.is_empty() {
                        let text_node = NodeData::text(text_str.as_str());
                        let text_dom = Dom {
                            root: text_node,
                            children: Vec::new().into(),
                            estimated_total_children: 0,
                        };
                        dom = dom.with_child(text_dom);
                    }
                }
                
                // Apply default CSS if provided
                let css = if $default_css.is_empty() {
                    azul_css::parser2::CssApiWrapper::empty()
                } else {
                    azul_css::parser2::CssApiWrapper::from_string($default_css.into())
                };
                
                Ok(dom.style(css))
            }

            fn compile_to_rust_code(
                &self,
                _: &XmlComponentMap,
                _: &ComponentArguments,
                _: &XmlTextContent,
            ) -> Result<String, CompileError> {
                Ok(format!("Dom::new(NodeType::{})", stringify!($node_type)))
            }

            fn get_xml_node(&self) -> XmlNode {
                self.node.clone()
            }
        }
    };
}

// Generate components for common HTML elements

// Structural elements
html_component!(HtmlRenderer, "html", NodeType::Html);
html_component!(HeadRenderer, "head", NodeType::Head);
html_component!(TitleRenderer, "title", NodeType::Title);
html_component!(BodyRenderer, "body", NodeType::Body);

// Block-level elements
html_component!(DivRenderer, "div", NodeType::Div);
html_component!(HeaderRenderer, "header", NodeType::Header);
html_component!(FooterRenderer, "footer", NodeType::Footer);
html_component!(SectionRenderer, "section", NodeType::Section);
html_component!(ArticleRenderer, "article", NodeType::Article);
html_component!(AsideRenderer, "aside", NodeType::Aside);
html_component!(NavRenderer, "nav", NodeType::Nav);
html_component!(MainRenderer, "main", NodeType::Main);

// Heading elements
html_component!(H1Renderer, "h1", NodeType::H1);
html_component!(H2Renderer, "h2", NodeType::H2);
html_component!(H3Renderer, "h3", NodeType::H3);
html_component!(H4Renderer, "h4", NodeType::H4);
html_component!(H5Renderer, "h5", NodeType::H5);
html_component!(H6Renderer, "h6", NodeType::H6);

// Text content elements
html_component!(PRenderer, "p", NodeType::P);
html_component!(SpanRenderer, "span", NodeType::Span);
html_component!(PreRenderer, "pre", NodeType::Pre);
html_component!(CodeRenderer, "code", NodeType::Code);
html_component!(BlockquoteRenderer, "blockquote", NodeType::BlockQuote);

// List elements
html_component!(UlRenderer, "ul", NodeType::Ul);
html_component!(OlRenderer, "ol", NodeType::Ol);
html_component!(LiRenderer, "li", NodeType::Li);
html_component!(DlRenderer, "dl", NodeType::Dl);
html_component!(DtRenderer, "dt", NodeType::Dt);
html_component!(DdRenderer, "dd", NodeType::Dd);

// Table elements
html_component!(TableRenderer, "table", NodeType::Table);
html_component!(TheadRenderer, "thead", NodeType::THead);
html_component!(TbodyRenderer, "tbody", NodeType::TBody);
html_component!(TfootRenderer, "tfoot", NodeType::TFoot);
html_component!(TrRenderer, "tr", NodeType::Tr);
html_component!(ThRenderer, "th", NodeType::Th);
html_component!(TdRenderer, "td", NodeType::Td);

// Inline elements
html_component!(ARenderer, "a", NodeType::A);
html_component!(StrongRenderer, "strong", NodeType::Strong);
html_component!(EmRenderer, "em", NodeType::Em);
html_component!(BRenderer, "b", NodeType::B);
html_component!(IRenderer, "i", NodeType::I);
html_component!(URenderer, "u", NodeType::U);
html_component!(SmallRenderer, "small", NodeType::Small);
html_component!(MarkRenderer, "mark", NodeType::Mark);
html_component!(SubRenderer, "sub", NodeType::Sub);
html_component!(SupRenderer, "sup", NodeType::Sup);

// Form elements
html_component!(FormRenderer, "form", NodeType::Form);
html_component!(LabelRenderer, "label", NodeType::Label);
html_component!(ButtonRenderer, "button", NodeType::Button);

// Other elements
html_component!(BrRenderer, "br", NodeType::Br);
html_component!(HrRenderer, "hr", NodeType::Hr);

/// Image component for rendering images (not yet fully implemented)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImgComponent {}

impl XmlComponentTrait for ImgComponent {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            accepts_text: false,
            args: vec![
                ("src".to_string(), "String".to_string()),
                ("alt".to_string(), "String".to_string()),
            ],
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _arguments: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        // For now, just return an empty div - image rendering needs proper ImageRef support
        Ok(Dom::div().style(azul_css::parser2::CssApiWrapper::empty()))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok("Dom::div() /* Image not yet supported */".to_string())
    }

    fn get_xml_node(&self) -> XmlNode {
        XmlNode::new("img")
    }
}

/// Creates and returns a component map with all default HTML components registered
pub fn printpdf_default_components() -> XmlComponentMap {
    let mut map = XmlComponentMap {
        components: Vec::new(),
    };

    // Register structural elements
    map.register_component(XmlComponent {
        id: normalize_casing("html"),
        renderer: Box::new(HtmlRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("head"),
        renderer: Box::new(HeadRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("title"),
        renderer: Box::new(TitleRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("body"),
        renderer: Box::new(BodyRenderer::new()),
        inherit_vars: true,
    });

    // Register block-level elements
    map.register_component(XmlComponent {
        id: normalize_casing("div"),
        renderer: Box::new(DivRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("header"),
        renderer: Box::new(HeaderRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("footer"),
        renderer: Box::new(FooterRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("section"),
        renderer: Box::new(SectionRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("article"),
        renderer: Box::new(ArticleRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("aside"),
        renderer: Box::new(AsideRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("nav"),
        renderer: Box::new(NavRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("main"),
        renderer: Box::new(MainRenderer::new()),
        inherit_vars: true,
    });

    // Register heading elements (h1-h6)
    map.register_component(XmlComponent {
        id: normalize_casing("h1"),
        renderer: Box::new(H1Renderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("h2"),
        renderer: Box::new(H2Renderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("h3"),
        renderer: Box::new(H3Renderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("h4"),
        renderer: Box::new(H4Renderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("h5"),
        renderer: Box::new(H5Renderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("h6"),
        renderer: Box::new(H6Renderer::new()),
        inherit_vars: true,
    });

    // Register text content elements
    map.register_component(XmlComponent {
        id: normalize_casing("p"),
        renderer: Box::new(PRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("span"),
        renderer: Box::new(SpanRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("pre"),
        renderer: Box::new(PreRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("code"),
        renderer: Box::new(CodeRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("blockquote"),
        renderer: Box::new(BlockquoteRenderer::new()),
        inherit_vars: true,
    });

    // Register inline formatting elements
    map.register_component(XmlComponent {
        id: normalize_casing("strong"),
        renderer: Box::new(StrongRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("em"),
        renderer: Box::new(EmRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("b"),
        renderer: Box::new(BRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("i"),
        renderer: Box::new(IRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("u"),
        renderer: Box::new(URenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("small"),
        renderer: Box::new(SmallRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("mark"),
        renderer: Box::new(MarkRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("sub"),
        renderer: Box::new(SubRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("sup"),
        renderer: Box::new(SupRenderer::new()),
        inherit_vars: true,
    });

    // Register list elements
    map.register_component(XmlComponent {
        id: normalize_casing("ul"),
        renderer: Box::new(UlRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("ol"),
        renderer: Box::new(OlRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("li"),
        renderer: Box::new(LiRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("dl"),
        renderer: Box::new(DlRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("dt"),
        renderer: Box::new(DtRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("dd"),
        renderer: Box::new(DdRenderer::new()),
        inherit_vars: true,
    });

    // Register table elements
    map.register_component(XmlComponent {
        id: normalize_casing("table"),
        renderer: Box::new(TableRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("thead"),
        renderer: Box::new(TheadRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("tbody"),
        renderer: Box::new(TbodyRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("tfoot"),
        renderer: Box::new(TfootRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("tr"),
        renderer: Box::new(TrRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("th"),
        renderer: Box::new(ThRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("td"),
        renderer: Box::new(TdRenderer::new()),
        inherit_vars: true,
    });

    // Register form elements
    map.register_component(XmlComponent {
        id: normalize_casing("form"),
        renderer: Box::new(FormRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("label"),
        renderer: Box::new(LabelRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("button"),
        renderer: Box::new(ButtonRenderer::new()),
        inherit_vars: true,
    });

    // Register link elements
    map.register_component(XmlComponent {
        id: normalize_casing("a"),
        renderer: Box::new(ARenderer::new()),
        inherit_vars: true,
    });

    // Register other elements
    map.register_component(XmlComponent {
        id: normalize_casing("br"),
        renderer: Box::new(BrRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("hr"),
        renderer: Box::new(HrRenderer::new()),
        inherit_vars: true,
    });

    // Register img component
    map.register_component(XmlComponent {
        id: "img".to_string(),
        renderer: Box::new(ImgComponent {}),
        inherit_vars: false,
    });

    map
}
