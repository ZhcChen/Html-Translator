use std::collections::BTreeMap;

use kuchiki::{NodeData, NodeRef, traits::TendrilSink};

use crate::model::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceDocument {
    pub nodes: Vec<SourceNode>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceNode {
    Element(SourceElement),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceElement {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<SourceNode>,
}

pub(crate) fn parse_source(html: &str, diagnostics: &mut Vec<Diagnostic>) -> SourceDocument {
    let document = kuchiki::parse_html().one(html);
    let mut source = SourceDocument {
        nodes: Vec::new(),
        styles: Vec::new(),
    };

    collect_children(
        &document,
        &mut source.nodes,
        &mut source.styles,
        diagnostics,
    );
    source
}

fn collect_children(
    parent: &NodeRef,
    output: &mut Vec<SourceNode>,
    styles: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for child in parent.children() {
        collect_node(&child, output, styles, diagnostics);
    }
}

fn collect_node(
    node: &NodeRef,
    output: &mut Vec<SourceNode>,
    styles: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node.data() {
        NodeData::Document(_) | NodeData::DocumentFragment => {
            collect_children(node, output, styles, diagnostics);
        }
        NodeData::Text(contents) => {
            let text = contents.borrow().to_string();
            if !is_formatting_whitespace(&text) {
                output.push(SourceNode::Text(text));
            }
        }
        NodeData::Element(element) => {
            let tag = element.name.local.to_string().to_ascii_lowercase();
            match tag.as_str() {
                "html" | "body" => collect_children(node, output, styles, diagnostics),
                "head" => collect_head(node, styles, diagnostics),
                "style" => {
                    let style = node.text_contents();
                    if !style.trim().is_empty() {
                        styles.push(style);
                    }
                }
                "script" => diagnostics.push(Diagnostic::warning(
                    "SCRIPT_OMITTED",
                    "<script> 不会被自动迁移；请手动实现目标平台逻辑。",
                    Some("script".to_string()),
                )),
                _ => {
                    let mut attributes = BTreeMap::new();
                    for (name, attribute) in &element.attributes.borrow().map {
                        attributes.insert(
                            name.local.to_string().to_ascii_lowercase(),
                            attribute.value.clone(),
                        );
                    }

                    let mut children = Vec::new();
                    collect_children(node, &mut children, styles, diagnostics);
                    output.push(SourceNode::Element(SourceElement {
                        tag,
                        attributes,
                        children,
                    }));
                }
            }
        }
        NodeData::Comment(_) | NodeData::Doctype(_) | NodeData::ProcessingInstruction(_) => {}
    }
}

fn collect_head(node: &NodeRef, styles: &mut Vec<String>, diagnostics: &mut Vec<Diagnostic>) {
    for child in node.children() {
        if let NodeData::Element(element) = child.data() {
            let tag = element.name.local.to_string().to_ascii_lowercase();
            match tag.as_str() {
                "style" => {
                    let style = child.text_contents();
                    if !style.trim().is_empty() {
                        styles.push(style);
                    }
                }
                "script" => diagnostics.push(Diagnostic::warning(
                    "SCRIPT_OMITTED",
                    "<script> 不会被自动迁移；请手动实现目标平台逻辑。",
                    Some("script".to_string()),
                )),
                "link" => diagnostics.push(Diagnostic::warning(
                    "EXTERNAL_STYLESHEET_OMITTED",
                    "外链样式不会被自动获取或迁移；请在目标工程中手动引入。",
                    Some("link".to_string()),
                )),
                "meta" | "title" => {}
                _ => diagnostics.push(Diagnostic::warning(
                    "HEAD_ELEMENT_OMITTED",
                    format!("<head> 中的 <{tag}> 未转换。"),
                    Some(tag),
                )),
            }
        }
    }
}

fn is_formatting_whitespace(text: &str) -> bool {
    text.trim().is_empty() && text.contains('\n')
}
