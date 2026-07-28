use crate::parser::{SourceElement, SourceNode};

pub(crate) const VOID_HTML_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const INLINE_TEXT_TAGS: &[&str] = &["b", "code", "em", "i", "small", "span", "strong"];

pub(crate) fn is_void_html_tag(tag: &str) -> bool {
    VOID_HTML_TAGS.contains(&tag)
}

pub(crate) fn is_boolean_attribute(name: &str) -> bool {
    matches!(
        name,
        "allowfullscreen"
            | "async"
            | "autofocus"
            | "autoplay"
            | "checked"
            | "controls"
            | "default"
            | "defer"
            | "disabled"
            | "formnovalidate"
            | "hidden"
            | "loop"
            | "multiple"
            | "muted"
            | "novalidate"
            | "open"
            | "readonly"
            | "required"
            | "reversed"
            | "selected"
    )
}

pub(crate) fn event_name(name: &str) -> Option<&'static str> {
    match name {
        "onclick" => Some("click"),
        "ondblclick" => Some("doubleClick"),
        "oninput" => Some("input"),
        "onchange" => Some("change"),
        "onsubmit" => Some("submit"),
        "onfocus" => Some("focus"),
        "onblur" => Some("blur"),
        _ => None,
    }
}

const JAVASCRIPT_RESERVED_WORDS: &[&str] = &[
    "abstract",
    "any",
    "arguments",
    "as",
    "asserts",
    "async",
    "await",
    "bigint",
    "boolean",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "constructor",
    "continue",
    "debugger",
    "declare",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "infer",
    "instanceof",
    "interface",
    "is",
    "keyof",
    "let",
    "module",
    "namespace",
    "never",
    "new",
    "null",
    "number",
    "object",
    "of",
    "override",
    "package",
    "private",
    "protected",
    "public",
    "readonly",
    "return",
    "satisfies",
    "static",
    "string",
    "super",
    "switch",
    "symbol",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "unique",
    "unknown",
    "using",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

pub(crate) fn is_handler_identifier(value: &str) -> bool {
    let identifier = value.trim();
    let mut characters = identifier.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }

    characters
        .all(|character| character == '_' || character == '$' || character.is_ascii_alphanumeric())
        && !JAVASCRIPT_RESERVED_WORDS.contains(&identifier)
}

pub(crate) fn is_text_only(element: &SourceElement) -> bool {
    element.children.iter().all(is_inline_text_node)
}

fn is_inline_text_node(node: &SourceNode) -> bool {
    match node {
        SourceNode::Text(_) => true,
        SourceNode::Element(element) => {
            INLINE_TEXT_TAGS.contains(&element.tag.as_str()) && is_text_only(element)
        }
    }
}

pub(crate) fn wechat_tag(element: &SourceElement) -> &'static str {
    match element.tag.as_str() {
        "a" => "navigator",
        "b" | "code" | "em" | "i" | "small" | "span" | "strong" if is_text_only(element) => "text",
        "button" => "button",
        "img" => "image",
        "input" => "input",
        "textarea" => "textarea",
        "audio" => "audio",
        "video" => "video",
        _ => "view",
    }
}

pub(crate) fn has_direct_wechat_mapping(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "article"
            | "aside"
            | "audio"
            | "b"
            | "blockquote"
            | "button"
            | "code"
            | "div"
            | "em"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "i"
            | "img"
            | "input"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "section"
            | "small"
            | "span"
            | "strong"
            | "textarea"
            | "ul"
            | "video"
    )
}

pub(crate) fn react_attribute_name(name: &str) -> &str {
    match name {
        "class" => "className",
        "for" => "htmlFor",
        "readonly" => "readOnly",
        "maxlength" => "maxLength",
        "tabindex" => "tabIndex",
        _ => name,
    }
}

pub(crate) fn vue_event_name(event: &str) -> &str {
    match event {
        "doubleClick" => "dblclick",
        event => event,
    }
}

pub(crate) fn wechat_event_name(event: &str) -> Option<&'static str> {
    match event {
        "click" => Some("tap"),
        "input" => Some("input"),
        "change" => Some("change"),
        "submit" => Some("submit"),
        "focus" => Some("focus"),
        "blur" => Some("blur"),
        _ => None,
    }
}

pub(crate) fn is_wechat_attribute_allowed(output_tag: &str, name: &str) -> bool {
    if matches!(name, "class" | "id" | "style" | "hidden") || name.starts_with("data-") {
        return true;
    }

    match output_tag {
        "image" => matches!(
            name,
            "src" | "mode" | "lazy-load" | "show-menu-by-longpress"
        ),
        "button" => matches!(
            name,
            "type" | "size" | "plain" | "disabled" | "loading" | "form-type"
        ),
        "input" => matches!(
            name,
            "type"
                | "value"
                | "name"
                | "password"
                | "placeholder"
                | "placeholder-style"
                | "placeholder-class"
                | "disabled"
                | "maxlength"
                | "focus"
                | "confirm-type"
        ),
        "textarea" => matches!(
            name,
            "value"
                | "name"
                | "placeholder"
                | "placeholder-style"
                | "placeholder-class"
                | "disabled"
                | "maxlength"
                | "focus"
                | "auto-height"
        ),
        "navigator" => matches!(name, "url" | "open-type" | "delta" | "hover-class"),
        "audio" | "video" => matches!(
            name,
            "src" | "poster" | "controls" | "autoplay" | "loop" | "muted"
        ),
        _ => false,
    }
}
