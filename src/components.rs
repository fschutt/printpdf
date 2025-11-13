use azul_core::{
    dom::Dom,
    styled_dom::StyledDom,
    xml::{
        normalize_casing, prepare_string, CompileError, ComponentArgumentTypes, ComponentArguments,
        FilteredComponentArguments, RenderDomError, XmlComponent, XmlComponentMap,
        XmlComponentTrait, XmlNode, XmlTextContent,
    },
};
use azul_css::props::parse::CssApiWrapper;
use serde_derive::{Deserialize, Serialize};

use crate::{ImageTypeInfo, RawImage, RawImageData, RawImageFormat};

/// Macro to generate HTML element components
/// Each HTML tag becomes a component that renders the corresponding DOM node
macro_rules! html_component {
    ($name:ident, $tag:expr, $dom_method:ident) => {
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
                // Create the DOM node
                let mut dom = Dom::$dom_method();
                
                // Add text content if present
                if let Some(text_str) = text.get_text() {
                    if !text_str.is_empty() {
                        dom = dom.with_child(Dom::text(text_str));
                    }
                }
                
                Ok(dom.style(CssApiWrapper::empty()))
            }

            fn compile_to_rust_code(
                &self,
                _: &XmlComponentMap,
                _: &ComponentArguments,
                _: &XmlTextContent,
            ) -> Result<String, CompileError> {
                Ok(format!("Dom::{}()", stringify!($dom_method)))
            }

            fn get_xml_node(&self) -> XmlNode {
                self.node.clone()
            }
        }
    };
}

// Generate components for common HTML elements
// Block-level elements
html_component!(DivRenderer, "div", div);
html_component!(BodyRenderer, "body", body);
html_component!(HeaderRenderer, "header", header);
html_component!(FooterRenderer, "footer", footer);
html_component!(SectionRenderer, "section", section);
html_component!(ArticleRenderer, "article", article);
html_component!(AsideRenderer, "aside", aside);
html_component!(NavRenderer, "nav", nav);
html_component!(MainRenderer, "main", main);

// Heading elements
html_component!(H1Renderer, "h1", h1);
html_component!(H2Renderer, "h2", h2);
html_component!(H3Renderer, "h3", h3);
html_component!(H4Renderer, "h4", h4);
html_component!(H5Renderer, "h5", h5);
html_component!(H6Renderer, "h6", h6);

// Text content elements
html_component!(PRenderer, "p", p);
html_component!(SpanRenderer, "span", span);
html_component!(PreRenderer, "pre", pre);
html_component!(CodeRenderer, "code", code);
html_component!(BlockquoteRenderer, "blockquote", blockquote);

// List elements
html_component!(UlRenderer, "ul", ul);
html_component!(OlRenderer, "ol", ol);
html_component!(LiRenderer, "li", li);
html_component!(DlRenderer, "dl", dl);
html_component!(DtRenderer, "dt", dt);
html_component!(DdRenderer, "dd", dd);

// Table elements
html_component!(TableRenderer, "table", table);
html_component!(TheadRenderer, "thead", thead);
html_component!(TbodyRenderer, "tbody", tbody);
html_component!(TfootRenderer, "tfoot", tfoot);
html_component!(TrRenderer, "tr", tr);
html_component!(ThRenderer, "th", th);
html_component!(TdRenderer, "td", td);

// Inline elements
html_component!(ARenderer, "a", a);
html_component!(StrongRenderer, "strong", strong);
html_component!(EmRenderer, "em", em);
html_component!(BRenderer, "b", b);
html_component!(IRenderer, "i", i);
html_component!(URenderer, "u", u);
html_component!(SmallRenderer, "small", small);
html_component!(MarkRenderer, "mark", mark);
html_component!(SubRenderer, "sub", sub);
html_component!(SupRenderer, "sup", sup);

// Form elements
html_component!(FormRenderer, "form", form);
html_component!(LabelRenderer, "label", label);
html_component!(ButtonRenderer, "button", button);
html_component!(FieldsetRenderer, "fieldset", fieldset);
html_component!(LegendRenderer, "legend", legend);

// Other elements
html_component!(BrRenderer, "br", br);
html_component!(HrRenderer, "hr", hr);

// HTML and head elements (for completeness)
html_component!(HtmlRenderer, "html", html);
html_component!(HeadRenderer, "head", head);
html_component!(TitleRenderer, "title", title);

/// Text renderer - kept for backward compatibility with existing code
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextRenderer {
    node: XmlNode,
    css: String,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("p"),
            css: String::new(),
        }
    }

    pub fn with_css(node_type: &str, css: &str) -> Self {
        Self {
            node: XmlNode::new(node_type),
            css: css.to_string(),
        }
    }
}

impl XmlComponentTrait for TextRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();

        if self.css.is_empty() {
            Ok(Dom::text(content).style(CssApiWrapper::empty()))
        } else {
            Ok(Dom::text(content).style(CssApiWrapper::from_string(self.css.clone().into())))
        }
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for horizontal rule (hr) element
pub struct HrRenderer {
    node: XmlNode,
}

impl HrRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("hr"),
        }
    }
}

impl XmlComponentTrait for HrRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "border: none; height: 1px; background-color: #ccc; margin: 10px 0; width: 100%;"
                .into(),
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table element
pub struct TableRenderer {
    node: XmlNode,
}

impl TableRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("table"),
        }
    }
}

impl XmlComponentTrait for TableRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: column; border: 1px solid #ccc; width: 100%;".into(),
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table row (tr) element
pub struct TrRenderer {
    node: XmlNode,
}

impl TrRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("tr"),
        }
    }
}

impl XmlComponentTrait for TrRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: row; width: 100%;".into(),
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table header (th) element
pub struct ThRenderer {
    node: XmlNode,
}

impl ThRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("th"),
        }
    }
}

impl XmlComponentTrait for ThRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();

        let css =
            "padding: 8px; border: 1px solid #ccc; font-weight: bold; text-align: left; flex: 1;";
        let mut dom = Dom::div().style(CssApiWrapper::from_string(css.into()));

        if !content.is_empty() {
            dom.append_child(Dom::text(content).style(CssApiWrapper::empty()));
        }

        Ok(dom)
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table data (td) element
pub struct TdRenderer {
    node: XmlNode,
}

impl TdRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("td"),
        }
    }
}

impl XmlComponentTrait for TdRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();

        let css = "padding: 8px; border: 1px solid #ccc; flex: 1;";
        let mut dom = Dom::div().style(CssApiWrapper::from_string(css.into()));

        if !content.is_empty() {
            dom.append_child(Dom::text(content).style(CssApiWrapper::empty()));
        }

        Ok(dom)
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for unordered list (ul) element
pub struct UlRenderer {
    node: XmlNode,
}

impl UlRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("ul"),
        }
    }
}

impl XmlComponentTrait for UlRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: column; padding-left: 20px; margin: 16px 0;".into(),
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for ordered list (ol) element
pub struct OlRenderer {
    node: XmlNode,
}

impl OlRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("ol"),
        }
    }
}

impl XmlComponentTrait for OlRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: column; padding-left: 20px; margin: 16px 0;".into(),
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Type of list for list items
pub enum ListType {
    Unordered,
    Ordered,
}

/// Renderer for list item (li) element
pub struct LiRenderer {
    node: XmlNode,
    list_type: ListType,
}

impl LiRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("li"),
            list_type: ListType::Unordered,
        }
    }

    pub fn with_list_type(list_type: ListType) -> Self {
        Self {
            node: XmlNode::new("li"),
            list_type,
        }
    }
}

impl XmlComponentTrait for LiRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();

        // Main container div for the list item (with flexbox row)
        let mut item_container = Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: row; align-items: flex-start; margin: 4px 0;".into(),
        ));

        // Bullet point div
        let bullet_text = match self.list_type {
            ListType::Unordered => "•",
            ListType::Ordered => "1.", // This would ideally be a counter in real CSS
        };

        let bullet_div =
            Dom::div()
                .with_child(Dom::text(bullet_text))
                .style(CssApiWrapper::from_string(
                    "width: 20px; flex-shrink: 0; text-align: center; margin-right: 5px;".into(),
                ));

        // Content div
        let mut content_div = Dom::div().style(CssApiWrapper::from_string("flex-grow: 1;".into()));

        if !content.is_empty() {
            content_div.append_child(Dom::text(content).style(CssApiWrapper::empty()));
        }

        // Add the bullet and content divs to the container
        item_container.append_child(bullet_div);
        item_container.append_child(content_div);

        Ok(item_container)
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for strong (bold text) element
pub struct StrongRenderer {
    node: XmlNode,
}

impl StrongRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("strong"),
        }
    }
}

impl XmlComponentTrait for StrongRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();

        Ok(Dom::text(content).style(CssApiWrapper::from_string("font-weight: bold;".into())))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for em (italic text) element
pub struct EmRenderer {
    node: XmlNode,
}

impl EmRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("em"),
        }
    }
}

impl XmlComponentTrait for EmRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();

        Ok(Dom::text(content).style(CssApiWrapper::from_string("font-style: italic;".into())))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Render for an `img` component
pub struct ImgComponent {}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub original_id: String,
    pub xobject_id: String,
    pub image_type: ImageTypeInfo,
    pub width: usize,
    pub height: usize,
}

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
        arguments: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let im_info = arguments
            .values
            .get("src")
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();

        let image_info = serde_json::from_slice::<ImageInfo>(&im_info)
            .ok()
            .unwrap_or_default();
        let data_format = RawImageFormat::RGB8;

        let image = RawImage {
            width: image_info.width,
            height: image_info.height,
            data_format,
            pixels: RawImageData::empty(data_format),
            tag: im_info,
        };

        let im = Dom::image(image.to_internal()).style(CssApiWrapper::empty());

        Ok(im)
    }
}

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
    map.register_component(XmlComponent {
        id: normalize_casing("fieldset"),
        renderer: Box::new(FieldsetRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("legend"),
        renderer: Box::new(LegendRenderer::new()),
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
