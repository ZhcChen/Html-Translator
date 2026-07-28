use std::collections::BTreeSet;

use crate::{
    model::{Diagnostic, GeneratedFile, Target},
    parser::{SourceDocument, SourceElement, SourceNode},
    rules::{
        event_name, has_direct_wechat_mapping, is_boolean_attribute, is_handler_identifier,
        is_text_only, is_void_html_tag, is_wechat_attribute_allowed, react_attribute_name,
        vue_event_name, wechat_event_name, wechat_tag,
    },
};

pub(crate) fn render(
    source: &SourceDocument,
    target: Target,
    component_name: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<GeneratedFile> {
    let names = ComponentNames::new(component_name, diagnostics);
    match target {
        Target::Vue => render_vue(source, &names, diagnostics),
        Target::React => render_react(source, &names, diagnostics),
        Target::WechatMiniProgram => render_wechat(source, &names, diagnostics),
    }
}

struct ComponentNames {
    pascal: String,
    kebab: String,
}

impl ComponentNames {
    fn new(requested: Option<&str>, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let raw = requested.unwrap_or("TranslatedComponent").trim();
        let words: Vec<&str> = raw
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();

        let mut pascal = String::new();
        for word in words {
            let mut characters = word.chars();
            if let Some(first) = characters.next() {
                pascal.push(first.to_ascii_uppercase());
                pascal.extend(characters);
            }
        }

        let has_valid_start = pascal
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic());
        if pascal.is_empty() || !has_valid_start {
            if requested.is_some() {
                diagnostics.push(Diagnostic::warning(
                    "COMPONENT_NAME_NORMALIZED",
                    format!(
                        "组件名 {raw:?} 不能生成安全的目标文件名，已改用 TranslatedComponent。"
                    ),
                    Some("componentName".to_string()),
                ));
            }
            pascal = "TranslatedComponent".to_string();
        }

        let mut kebab = String::new();
        for (index, character) in pascal.chars().enumerate() {
            if character.is_ascii_uppercase() && index > 0 {
                kebab.push('-');
            }
            kebab.push(character.to_ascii_lowercase());
        }

        Self { pascal, kebab }
    }
}

fn render_vue(
    source: &SourceDocument,
    names: &ComponentNames,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<GeneratedFile> {
    let mut template = String::from("<template>\n");
    let handlers = {
        let mut renderer = VueRenderer {
            diagnostics,
            handlers: BTreeSet::new(),
        };
        renderer.render_nodes(&source.nodes, &mut template, 1);
        renderer.handlers
    };
    template.push_str("</template>\n");

    if !handlers.is_empty() {
        template.push_str("\n<script setup>\n");
        for handler in handlers {
            template.push_str("function ");
            template.push_str(&handler);
            template.push_str("() {}\n");
        }
        template.push_str("</script>\n");
    }

    let styles = join_styles(source);
    if !styles.is_empty() {
        template.push_str("\n<style>\n");
        template.push_str(&styles);
        if !styles.ends_with('\n') {
            template.push('\n');
        }
        template.push_str("</style>\n");
    }

    vec![GeneratedFile {
        path: format!("{}.vue", names.pascal),
        content: template,
    }]
}

struct VueRenderer<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    handlers: BTreeSet<String>,
}

impl VueRenderer<'_> {
    fn render_nodes(&mut self, nodes: &[SourceNode], output: &mut String, depth: usize) {
        for node in nodes {
            self.render_node(node, output, depth);
        }
    }

    fn render_node(&mut self, node: &SourceNode, output: &mut String, depth: usize) {
        match node {
            SourceNode::Text(text) if contains_template_binding(text) => {
                record_template_binding_diagnostic(self.diagnostics, "Vue", "text");
                indent(output, depth);
                output.push_str("<span");
                append_vue_text_literal(output, text);
                output.push_str("></span>\n");
            }
            SourceNode::Text(text) => {
                indent(output, depth);
                output.push_str(&escape_template_text(text));
                output.push('\n');
            }
            SourceNode::Element(element) => self.render_element(element, output, depth),
        }
    }

    fn render_element(&mut self, element: &SourceElement, output: &mut String, depth: usize) {
        let literal_text =
            direct_text_child(element).filter(|text| contains_template_binding(text));
        let mut attributes = self.render_attributes(element);
        if let Some(text) = literal_text {
            record_template_binding_diagnostic(self.diagnostics, "Vue", "text");
            append_vue_text_literal(&mut attributes, text);
        }

        indent(output, depth);
        output.push('<');
        output.push_str(&element.tag);
        output.push_str(&attributes);

        if is_void_html_tag(&element.tag) {
            output.push_str(" />\n");
            return;
        }

        if let Some(text) = direct_text_child(element) {
            if literal_text.is_some() {
                output.push_str("></");
                output.push_str(&element.tag);
                output.push_str(">\n");
                return;
            }
            output.push('>');
            output.push_str(&escape_template_text(text));
            output.push_str("</");
            output.push_str(&element.tag);
            output.push_str(">\n");
            return;
        }

        if element.children.is_empty() {
            output.push_str("></");
            output.push_str(&element.tag);
            output.push_str(">\n");
            return;
        }

        output.push_str(">\n");
        self.render_nodes(&element.children, output, depth + 1);
        indent(output, depth);
        output.push_str("</");
        output.push_str(&element.tag);
        output.push_str(">\n");
    }

    fn render_attributes(&mut self, element: &SourceElement) -> String {
        let mut attributes = String::new();
        for (name, value) in &element.attributes {
            if let Some(event) = event_name(name) {
                if is_handler_identifier(value) {
                    self.record_handler(value, name);
                    attributes.push_str(" @");
                    attributes.push_str(vue_event_name(event));
                    attributes.push_str("=\"");
                    attributes.push_str(&escape_attribute(value));
                    attributes.push('"');
                } else {
                    self.unsupported_event(name, value);
                }
                continue;
            }
            if name.starts_with("on") {
                self.unsupported_event(name, value);
                continue;
            }
            if contains_template_binding(value) {
                record_template_binding_diagnostic(self.diagnostics, "Vue", name);
                append_template_literal_attribute(&mut attributes, name, value);
            } else {
                append_html_attribute(&mut attributes, name, value);
            }
        }
        attributes
    }

    fn record_handler(&mut self, handler: &str, context: &str) {
        if self.handlers.insert(handler.to_string()) {
            self.diagnostics.push(Diagnostic::warning(
                "EVENT_HANDLER_STUBBED",
                format!("事件处理器 {handler} 已生成空函数；请迁移原始业务逻辑。"),
                Some(context.to_string()),
            ));
        }
    }

    fn unsupported_event(&mut self, name: &str, value: &str) {
        self.diagnostics.push(Diagnostic::warning(
            "EVENT_EXPRESSION_UNSUPPORTED",
            format!("{name}=\"{value}\" 不是可安全转换的函数标识符。"),
            Some(name.to_string()),
        ));
    }
}

fn render_react(
    source: &SourceDocument,
    names: &ComponentNames,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<GeneratedFile> {
    let mut markup = String::new();
    let handlers = {
        let mut renderer = ReactRenderer {
            diagnostics,
            handlers: BTreeSet::new(),
        };
        renderer.render_nodes(&source.nodes, &mut markup, 3);
        renderer.handlers
    };

    let mut component = format!("import \"./{}.css\";\n\n", names.pascal);
    component.push_str("export default function ");
    component.push_str(&names.pascal);
    component.push_str("() {\n");
    for handler in handlers {
        component.push_str("  const ");
        component.push_str(&handler);
        component.push_str(" = () => {};\n");
    }
    if !component.ends_with("{\n") {
        component.push('\n');
    }
    component.push_str("  return (\n    <>\n");
    component.push_str(&markup);
    component.push_str("    </>\n  );\n}\n");

    vec![
        GeneratedFile {
            path: format!("{}.tsx", names.pascal),
            content: component,
        },
        GeneratedFile {
            path: format!("{}.css", names.pascal),
            content: join_styles(source),
        },
    ]
}

struct ReactRenderer<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    handlers: BTreeSet<String>,
}

impl ReactRenderer<'_> {
    fn render_nodes(&mut self, nodes: &[SourceNode], output: &mut String, depth: usize) {
        for node in nodes {
            self.render_node(node, output, depth);
        }
    }

    fn render_node(&mut self, node: &SourceNode, output: &mut String, depth: usize) {
        match node {
            SourceNode::Text(text) => {
                indent(output, depth);
                output.push_str(&escape_jsx_text(text));
                output.push('\n');
            }
            SourceNode::Element(element) => self.render_element(element, output, depth),
        }
    }

    fn render_element(&mut self, element: &SourceElement, output: &mut String, depth: usize) {
        let attributes = self.render_attributes(element);
        indent(output, depth);
        output.push('<');
        output.push_str(&element.tag);
        output.push_str(&attributes);

        if is_void_html_tag(&element.tag) {
            output.push_str(" />\n");
            return;
        }

        if let Some(text) = direct_text_child(element) {
            output.push('>');
            output.push_str(&escape_jsx_text(text));
            output.push_str("</");
            output.push_str(&element.tag);
            output.push_str(">\n");
            return;
        }

        if element.children.is_empty() {
            output.push_str("></");
            output.push_str(&element.tag);
            output.push_str(">\n");
            return;
        }

        output.push_str(">\n");
        self.render_nodes(&element.children, output, depth + 1);
        indent(output, depth);
        output.push_str("</");
        output.push_str(&element.tag);
        output.push_str(">\n");
    }

    fn render_attributes(&mut self, element: &SourceElement) -> String {
        let mut attributes = String::new();
        for (name, value) in &element.attributes {
            if let Some(event) = event_name(name) {
                if is_handler_identifier(value) {
                    self.record_handler(value, name);
                    attributes.push_str(" on");
                    attributes.push_str(event);
                    attributes.push_str("={");
                    attributes.push_str(value.trim());
                    attributes.push('}');
                } else {
                    self.unsupported_event(name, value);
                }
                continue;
            }
            if name.starts_with("on") {
                self.unsupported_event(name, value);
                continue;
            }
            if name == "style" {
                if let Some(style) = react_style(value, self.diagnostics) {
                    attributes.push_str(" style={");
                    attributes.push_str(&style);
                    attributes.push('}');
                }
                continue;
            }
            append_react_attribute(&mut attributes, name, value);
        }
        attributes
    }

    fn record_handler(&mut self, handler: &str, context: &str) {
        if self.handlers.insert(handler.to_string()) {
            self.diagnostics.push(Diagnostic::warning(
                "EVENT_HANDLER_STUBBED",
                format!("事件处理器 {handler} 已生成空函数；请迁移原始业务逻辑。"),
                Some(context.to_string()),
            ));
        }
    }

    fn unsupported_event(&mut self, name: &str, value: &str) {
        self.diagnostics.push(Diagnostic::warning(
            "EVENT_EXPRESSION_UNSUPPORTED",
            format!("{name}=\"{value}\" 不是可安全转换的函数标识符。"),
            Some(name.to_string()),
        ));
    }
}

fn render_wechat(
    source: &SourceDocument,
    names: &ComponentNames,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<GeneratedFile> {
    let mut markup = String::new();
    let handlers = {
        let mut renderer = WechatRenderer {
            diagnostics,
            handlers: BTreeSet::new(),
        };
        renderer.render_nodes(&source.nodes, &mut markup, 0);
        renderer.handlers
    };

    let styles = join_styles(source);
    if !styles.is_empty() {
        diagnostics.push(Diagnostic::warning(
            "WECHAT_CSS_REVIEW_REQUIRED",
            "CSS 已原样输出为 WXSS；请检查标签选择器和平台兼容性。",
            Some("style".to_string()),
        ));
    }

    let mut page = String::from("Page({");
    if handlers.is_empty() {
        page.push_str("});\n");
    } else {
        page.push('\n');
        for handler in handlers {
            page.push_str("  ");
            page.push_str(&handler);
            page.push_str("() {},\n");
        }
        page.push_str("});\n");
    }

    vec![
        GeneratedFile {
            path: format!("{}.wxml", names.kebab),
            content: markup,
        },
        GeneratedFile {
            path: format!("{}.wxss", names.kebab),
            content: styles,
        },
        GeneratedFile {
            path: format!("{}.js", names.kebab),
            content: page,
        },
        GeneratedFile {
            path: format!("{}.json", names.kebab),
            content: "{}\n".to_string(),
        },
    ]
}

struct WechatRenderer<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    handlers: BTreeSet<String>,
}

impl WechatRenderer<'_> {
    fn render_nodes(&mut self, nodes: &[SourceNode], output: &mut String, depth: usize) {
        for node in nodes {
            self.render_node(node, output, depth);
        }
    }

    fn render_node(&mut self, node: &SourceNode, output: &mut String, depth: usize) {
        match node {
            SourceNode::Text(text) if contains_template_binding(text) => {
                record_template_binding_diagnostic(self.diagnostics, "微信小程序", "text");
                indent(output, depth);
                output.push_str(&escape_template_literal_text(text));
                output.push('\n');
            }
            SourceNode::Text(text) => {
                indent(output, depth);
                output.push_str(&escape_template_text(text));
                output.push('\n');
            }
            SourceNode::Element(element) => self.render_element(element, output, depth),
        }
    }

    fn render_element(&mut self, element: &SourceElement, output: &mut String, depth: usize) {
        let tag = self.output_tag(element);
        let attributes = self.render_attributes(element, &tag);
        indent(output, depth);
        output.push('<');
        output.push_str(&tag);
        output.push_str(&attributes);

        if matches!(tag.as_str(), "image" | "input") {
            output.push_str(" />\n");
            return;
        }

        if let Some(text) = direct_text_child(element) {
            output.push('>');
            if contains_template_binding(text) {
                record_template_binding_diagnostic(self.diagnostics, "微信小程序", "text");
                output.push_str(&escape_template_literal_text(text));
            } else {
                output.push_str(&escape_template_text(text));
            }
            output.push_str("</");
            output.push_str(&tag);
            output.push_str(">\n");
            return;
        }

        if element.children.is_empty() {
            output.push_str("></");
            output.push_str(&tag);
            output.push_str(">\n");
            return;
        }

        output.push_str(">\n");
        self.render_nodes(&element.children, output, depth + 1);
        indent(output, depth);
        output.push_str("</");
        output.push_str(&tag);
        output.push_str(">\n");
    }

    fn output_tag(&mut self, element: &SourceElement) -> String {
        if element.tag == "a" {
            if let Some(href) = element.attributes.get("href") {
                if href.starts_with('/') {
                    return "navigator".to_string();
                }
                self.diagnostics.push(Diagnostic::warning(
                    "WECHAT_EXTERNAL_LINK_UNSUPPORTED",
                    format!("链接 {href} 不是可直接映射的小程序页面路径。"),
                    Some("href".to_string()),
                ));
            } else {
                self.diagnostics.push(Diagnostic::warning(
                    "WECHAT_LINK_WITHOUT_HREF",
                    "<a> 缺少 href，已降级为 view。",
                    Some("a".to_string()),
                ));
            }
            return "view".to_string();
        }

        if !has_direct_wechat_mapping(&element.tag) {
            self.diagnostics.push(Diagnostic::warning(
                "WECHAT_TAG_TO_VIEW",
                format!("<{0}> 没有直接 WXML 映射，已降级为 <view>。", element.tag),
                Some(element.tag.clone()),
            ));
        } else if matches!(
            element.tag.as_str(),
            "b" | "code" | "em" | "i" | "small" | "span" | "strong"
        ) && !is_text_only(element)
        {
            self.diagnostics.push(Diagnostic::warning(
                "WECHAT_INLINE_TO_VIEW",
                format!("复杂 <{}> 内容已降级为 <view>。", element.tag),
                Some(element.tag.clone()),
            ));
        }

        wechat_tag(element).to_string()
    }

    fn render_attributes(&mut self, element: &SourceElement, output_tag: &str) -> String {
        let mut attributes = String::new();
        for (name, value) in &element.attributes {
            if name == "href" && element.tag == "a" {
                if output_tag == "navigator" {
                    self.append_attribute(&mut attributes, "url", value);
                }
                continue;
            }
            if let Some(event) = event_name(name) {
                if let Some(wechat_event) = wechat_event_name(event) {
                    if is_handler_identifier(value) {
                        self.record_handler(value, name);
                        attributes.push_str(" bind");
                        attributes.push_str(wechat_event);
                        attributes.push_str("=\"");
                        attributes.push_str(&escape_attribute(value));
                        attributes.push('"');
                    } else {
                        self.unsupported_event(name, value);
                    }
                } else {
                    self.diagnostics.push(Diagnostic::warning(
                        "WECHAT_EVENT_UNSUPPORTED",
                        format!("{name} 不存在等价的小程序事件映射。"),
                        Some(name.to_string()),
                    ));
                }
                continue;
            }
            if name.starts_with("on") {
                self.unsupported_event(name, value);
                continue;
            }
            if name == "alt" && output_tag == "image" {
                self.diagnostics.push(Diagnostic::warning(
                    "WECHAT_IMAGE_ALT_OMITTED",
                    "小程序 image 没有等价的 alt 属性，已省略。",
                    Some(name.to_string()),
                ));
                continue;
            }
            if is_wechat_attribute_allowed(output_tag, name) {
                self.append_attribute(&mut attributes, name, value);
            } else {
                self.diagnostics.push(Diagnostic::warning(
                    "WECHAT_ATTRIBUTE_OMITTED",
                    format!("属性 {name} 不适用于生成的 <{output_tag}>，已省略。"),
                    Some(name.to_string()),
                ));
            }
        }
        attributes
    }

    fn append_attribute(&mut self, output: &mut String, name: &str, value: &str) {
        if contains_template_binding(value) {
            record_template_binding_diagnostic(self.diagnostics, "微信小程序", name);
            append_template_literal_attribute(output, name, value);
        } else {
            append_html_attribute(output, name, value);
        }
    }

    fn record_handler(&mut self, handler: &str, context: &str) {
        if self.handlers.insert(handler.to_string()) {
            self.diagnostics.push(Diagnostic::warning(
                "EVENT_HANDLER_STUBBED",
                format!("事件处理器 {handler} 已生成空函数；请迁移原始业务逻辑。"),
                Some(context.to_string()),
            ));
        }
    }

    fn unsupported_event(&mut self, name: &str, value: &str) {
        self.diagnostics.push(Diagnostic::warning(
            "EVENT_EXPRESSION_UNSUPPORTED",
            format!("{name}=\"{value}\" 不是可安全转换的函数标识符。"),
            Some(name.to_string()),
        ));
    }
}

fn join_styles(source: &SourceDocument) -> String {
    source
        .styles
        .iter()
        .map(|style| style.trim())
        .filter(|style| !style.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn direct_text_child(element: &SourceElement) -> Option<&str> {
    match element.children.as_slice() {
        [SourceNode::Text(text)] => Some(text),
        _ => None,
    }
}

fn append_html_attribute(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    if is_boolean_attribute(name) && (value.is_empty() || value == name) {
        return;
    }
    output.push_str("=\"");
    output.push_str(&escape_attribute(value));
    output.push('"');
}

fn append_react_attribute(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(react_attribute_name(name));
    if is_boolean_attribute(name) && (value.is_empty() || value == name) {
        return;
    }
    output.push_str("=\"");
    output.push_str(&escape_attribute(value));
    output.push('"');
}

fn react_style(value: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<String> {
    let mut properties = Vec::new();
    for declaration in split_top_level(value, ';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let Some((property, property_value)) = split_once_top_level(declaration, ':') else {
            diagnostics.push(Diagnostic::warning(
                "REACT_INLINE_STYLE_OMITTED",
                format!("无法解析内联样式声明 {declaration:?}，已省略。"),
                Some("style".to_string()),
            ));
            continue;
        };
        let property = property.trim();
        let property_value = property_value.trim();
        if property.is_empty() || property_value.is_empty() {
            diagnostics.push(Diagnostic::warning(
                "REACT_INLINE_STYLE_OMITTED",
                format!("无法解析内联样式声明 {declaration:?}，已省略。"),
                Some("style".to_string()),
            ));
            continue;
        }
        let Some(key) = react_style_property(property) else {
            diagnostics.push(Diagnostic::warning(
                "REACT_INLINE_STYLE_OMITTED",
                format!("样式属性 {property:?} 不能安全映射为 React style 对象键，已省略。"),
                Some("style".to_string()),
            ));
            continue;
        };
        let encoded = serde_json::to_string(property_value)
            .expect("serializing a Rust string to JSON must succeed");
        properties.push(format!("{key}: {encoded}"));
    }

    (!properties.is_empty()).then(|| format!("{{ {} }}", properties.join(", ")))
}

fn split_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut output = Vec::new();
    let mut start = 0;
    let mut nesting = 0_usize;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '\"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => nesting += 1,
            ')' => nesting = nesting.saturating_sub(1),
            _ if character == separator && nesting == 0 => {
                output.push(&value[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    output.push(&value[start..]);
    output
}

fn split_once_top_level(value: &str, separator: char) -> Option<(&str, &str)> {
    let mut nesting = 0_usize;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '\"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => nesting += 1,
            ')' => nesting = nesting.saturating_sub(1),
            _ if character == separator && nesting == 0 => {
                let next = index + character.len_utf8();
                return Some((&value[..index], &value[next..]));
            }
            _ => {}
        }
    }
    None
}

fn react_style_property(property: &str) -> Option<String> {
    if let Some(custom_property) = property.strip_prefix("--") {
        if custom_property.is_empty()
            || !custom_property.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return None;
        }
        return Some(
            serde_json::to_string(property)
                .expect("serializing a Rust string to JSON must succeed"),
        );
    }

    let is_vendor_property = property.starts_with('-');
    let property = property.strip_prefix('-').unwrap_or(property);
    if property.is_empty() {
        return None;
    }

    let mut output = String::new();
    for (index, segment) in property.split('-').enumerate() {
        if !is_valid_css_property_segment(segment) {
            return None;
        }
        if index == 0 && (!is_vendor_property || segment == "ms") {
            output.push_str(segment);
        } else {
            append_capitalized(&mut output, segment);
        }
    }

    (!output.is_empty()).then_some(output)
}

fn is_valid_css_property_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_alphabetic())
        && characters.all(|character| character.is_ascii_alphanumeric())
}

fn append_capitalized(output: &mut String, segment: &str) {
    let mut characters = segment.chars();
    if let Some(first) = characters.next() {
        output.push(first.to_ascii_uppercase());
        output.extend(characters);
    }
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('\"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn contains_template_binding(value: &str) -> bool {
    value.contains("{{") || value.contains("}}")
}

fn record_template_binding_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    target: &str,
    context: &str,
) {
    diagnostics.push(Diagnostic::warning(
        "TEMPLATE_BINDING_ESCAPED",
        format!("检测到字面 Mustache 语法，已在 {target} 输出中转义以避免被当作数据绑定。"),
        Some(context.to_string()),
    ));
}

fn append_vue_text_literal(output: &mut String, value: &str) {
    let serialized =
        serde_json::to_string(value).expect("serializing a Rust string to JSON must succeed");
    output.push_str(" v-text=\"");
    output.push_str(&escape_attribute(&serialized));
    output.push('"');
}

fn append_template_literal_attribute(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escape_template_literal_attribute(value));
    output.push('"');
}

fn escape_template_literal_attribute(value: &str) -> String {
    escape_attribute(value)
        .replace('{', "&#123;")
        .replace('}', "&#125;")
}

fn escape_template_literal_text(value: &str) -> String {
    escape_template_text(value)
        .replace('{', "&#123;")
        .replace('}', "&#125;")
}

fn escape_template_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_jsx_text(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '{' => output.push_str("{\"{\"}"),
            '}' => output.push_str("{\"}\"}"),
            _ => output.push(character),
        }
    }
    output
}

fn indent(output: &mut String, depth: usize) {
    output.push_str(&"  ".repeat(depth));
}
