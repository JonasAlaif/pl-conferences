use std::{collections::HashMap, error::Error, rc::Rc, sync::OnceLock};

use ollama_rs::generation::tools::implementations::DDGSearcher;
use serde::{Deserialize, Serialize};

pub async fn search(query: &str) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>> {
    let ddg = ddg_search();
    let results = ddg.search(query).await?;
    let results = serde_json::to_value(results).unwrap();
    Ok(serde_json::from_value(results).unwrap())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub link: String,
    pub snippet: String,
}

impl SearchResult {
    pub async fn scrape(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("https://{}", self.link))
            .send()
            .await?
            .text()
            .await?;
        Ok(html_to_md(&response))
    }
}

pub fn html_to_md(html: &str) -> String {
    use html2md::*;
    struct IgnoreHandler(Rc<dyn Fn(&NodeData) -> bool>, Option<Box<dyn TagHandler>>);
    impl TagHandler for IgnoreHandler {
        fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
            if self.0(&tag.data) {
                return;
            };
            let NodeData::Element { name, .. } = &tag.data else {
                unreachable!();
            };
            use html2md::{
                anchors::*, codes::*, containers::*, dummy::*, headers::*, iframes::*, images::*,
                lists::*, paragraphs::*, quotes::*, styles::*, tables::*,
            };
            let mut handler = match name.local.as_ref() {
                // containers
                "div" | "section" | "header" | "footer" => {
                    Box::new(ContainerHandler::default()) as Box<dyn TagHandler>
                }
                // pagination, breaks
                "p" | "br" | "hr" => Box::new(ParagraphHandler::default()),
                "q" | "cite" | "blockquote" => Box::new(QuoteHandler::default()),
                // spoiler tag
                "details" | "summary" => Box::new(HtmlCherryPickHandler::default()),
                // formatting
                "b" | "i" | "s" | "strong" | "em" | "del" => Box::new(StyleHandler::default()),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Box::new(HeaderHandler::default()),
                "pre" | "code" => Box::new(CodeHandler::default()),
                // images, links
                "img" => Box::new(ImgHandler::default()),
                "a" => Box::new(AnchorHandler::default()),
                // lists
                "ol" | "ul" | "menu" => Box::new(ListHandler::default()),
                "li" => Box::new(ListItemHandler::default()),
                // HAD TO CHANGE THE HANDLER HERE DUE TO NOT PUBLIC!
                // as-is
                "sub" | "sup" => Box::new(DummyHandler::default()),
                // tables, handled fully internally as markdown can't have nested content in tables
                // supports only single tables as of now
                "table" => Box::new(TableHandler::default()),
                "iframe" => Box::new(IframeHandler::default()),
                // other
                "html" | "head" | "body" => Box::new(DummyHandler::default()),
                _ => Box::new(DummyHandler::default()),
            };
            handler.handle(tag, printer);
            self.1 = Some(handler);
        }

        fn after_handle(&mut self, printer: &mut StructuredPrinter) {
            if let Some(handler) = &mut self.1 {
                handler.after_handle(printer);
            }
        }

        fn skip_descendants(&self) -> bool {
            self.1.is_none()
        }
    }

    struct IgnoreFactory(Rc<dyn Fn(&NodeData) -> bool>);
    impl TagHandlerFactory for IgnoreFactory {
        fn instantiate(&self) -> Box<dyn TagHandler> {
            Box::new(IgnoreHandler(Rc::clone(&self.0), None))
        }
    }

    let mut custom = HashMap::<String, Box<dyn TagHandlerFactory>>::new();
    custom.insert(
        "script".to_string(),
        Box::new(IgnoreFactory(Rc::new(|_| true))),
    );
    custom.insert(
        "ul".to_string(),
        Box::new(IgnoreFactory(Rc::new(|data| {
            let NodeData::Element { attrs, .. } = data else {
                unreachable!();
            };
            attrs.borrow().iter().any(|attr| {
                attr.name.local.as_ref() == "class" && attr.value.as_ref() == "list-group"
            })
        }))),
    );

    parse_html_custom(html, &custom)
}

fn ddg_search() -> &'static DDGSearcher {
    static DDG_SEARCH: OnceLock<DDGSearcher> = OnceLock::new();
    DDG_SEARCH.get_or_init(|| DDGSearcher::default())
}

#[tokio::test]
async fn search_oopsla_25() {
    let results = search("OOPSLA 2025 conference call for papers cfp")
        .await
        .unwrap();
    debug_assert_eq!(results[0].link, "2025.splashcon.org/track/OOPSLA");
}

#[test]
fn test_html_to_md() {
    let html = include_str!("../llm/tests/oopsla25.html");
    let md = html_to_md(html);
    assert_eq!(md, include_str!("../llm/tests/oopsla25.md"));
}
