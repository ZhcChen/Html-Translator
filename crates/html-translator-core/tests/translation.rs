use html_translator_core::{
    GeneratedFile, Target, TranslationRequest, TranslationResult, translate,
};

fn translate_html(html: &str, target: Target, component_name: Option<&str>) -> TranslationResult {
    translate(TranslationRequest {
        html: html.to_string(),
        target,
        component_name: component_name.map(str::to_string),
    })
    .expect("translation should succeed")
}

fn file<'a>(result: &'a TranslationResult, path: &str) -> &'a GeneratedFile {
    result
        .files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("missing generated file {path}"))
}

fn has_diagnostic(result: &TranslationResult, code: &str) -> bool {
    result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == code)
}

#[test]
fn generates_a_vue_sfc_with_a_handler_stub() {
    let result = translate_html(
        r#"<style>.card { color: red; }</style><div class="card"><button onclick="submit">Save</button></div>"#,
        Target::Vue,
        Some("profile-card"),
    );

    assert_eq!(result.files.len(), 1);
    let component = file(&result, "ProfileCard.vue");
    assert!(component.content.contains("<div class=\"card\">"));
    assert!(
        component
            .content
            .contains("<button @click=\"submit\">Save</button>")
    );
    assert!(component.content.contains("function submit() {}"));
    assert!(component.content.contains(".card { color: red; }"));
    assert!(has_diagnostic(&result, "EVENT_HANDLER_STUBBED"));
}

#[test]
fn generates_react_tsx_and_converts_react_specific_attributes() {
    let result = translate_html(
        r#"<label for="email"><input class="field" style="font-size: 12px; color: red" /></label>"#,
        Target::React,
        Some("account-form"),
    );

    assert_eq!(result.files.len(), 2);
    let component = file(&result, "AccountForm.tsx");
    assert!(component.content.contains("import \"./AccountForm.css\";"));
    assert!(component.content.contains("<label htmlFor=\"email\">"));
    assert!(
        component
            .content
            .contains("className=\"field\" style={{ fontSize: \"12px\", color: \"red\" }}")
    );
    assert!(file(&result, "AccountForm.css").content.is_empty());
}

#[test]
fn generates_a_complete_wechat_page_file_set() {
    let result = translate_html(
        r#"<style>.card { display: flex; }</style><section class="card"><span>Hello</span><img src="/logo.png" alt="Logo" /><a href="/pages/about/index">About</a><button onclick="submit">Save</button></section>"#,
        Target::WechatMiniProgram,
        Some("landing-page"),
    );

    let paths: Vec<&str> = result.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "landing-page.wxml",
            "landing-page.wxss",
            "landing-page.js",
            "landing-page.json",
        ]
    );

    let wxml = &file(&result, "landing-page.wxml").content;
    assert!(wxml.contains("<view class=\"card\">"));
    assert!(wxml.contains("<text>Hello</text>"));
    assert!(wxml.contains("<image src=\"/logo.png\" />"));
    assert!(wxml.contains("<navigator url=\"/pages/about/index\">About</navigator>"));
    assert!(wxml.contains("<button bindtap=\"submit\">Save</button>"));
    assert_eq!(
        file(&result, "landing-page.wxss").content,
        ".card { display: flex; }"
    );
    assert!(
        file(&result, "landing-page.js")
            .content
            .contains("submit() {}")
    );
    assert!(has_diagnostic(&result, "WECHAT_IMAGE_ALT_OMITTED"));
    assert!(has_diagnostic(&result, "WECHAT_CSS_REVIEW_REQUIRED"));
    assert!(has_diagnostic(&result, "EVENT_HANDLER_STUBBED"));
}

#[test]
fn omits_script_and_unsafe_event_expressions() {
    let result = translate_html(
        r#"<script>window.alert('no')</script><select onclick="submit(1)"><option>One</option></select>"#,
        Target::WechatMiniProgram,
        None,
    );

    let wxml = &file(&result, "translated-component.wxml").content;
    assert!(!wxml.contains("bindtap"));
    assert!(!wxml.contains("submit(1)"));
    assert!(has_diagnostic(&result, "SCRIPT_OMITTED"));
    assert!(has_diagnostic(&result, "EVENT_EXPRESSION_UNSUPPORTED"));
    assert!(has_diagnostic(&result, "WECHAT_TAG_TO_VIEW"));
}

#[test]
fn takes_visible_nodes_from_a_complete_html_document() {
    let result = translate_html(
        r#"<!doctype html><html><head><title>Ignored</title><style>.page { padding: 8px; }</style></head><body><main class="page">Body</main></body></html>"#,
        Target::Vue,
        None,
    );

    let component = &file(&result, "TranslatedComponent.vue").content;
    assert!(component.contains("<main class=\"page\">Body</main>"));
    assert!(!component.contains("<title>"));
    assert!(!component.contains("<body>"));
    assert!(component.contains(".page { padding: 8px; }"));
}

#[test]
fn normalizes_an_invalid_component_name() {
    let result = translate_html("<div />", Target::React, Some("123 !!!"));

    assert!(
        result
            .files
            .iter()
            .any(|file| file.path == "TranslatedComponent.tsx")
    );
    assert!(has_diagnostic(&result, "COMPONENT_NAME_NORMALIZED"));
}

#[test]
fn rejects_javascript_keywords_as_handler_names() {
    let vue = translate_html(r#"<button onclick="null">Save</button>"#, Target::Vue, None);
    let vue_component = &file(&vue, "TranslatedComponent.vue").content;
    assert!(!vue_component.contains("@click"));
    assert!(!vue_component.contains("function null"));
    assert!(has_diagnostic(&vue, "EVENT_EXPRESSION_UNSUPPORTED"));

    let wechat = translate_html(
        r#"<button onclick="await">Save</button>"#,
        Target::WechatMiniProgram,
        None,
    );
    let wxml = &file(&wechat, "translated-component.wxml").content;
    let page = &file(&wechat, "translated-component.js").content;
    assert!(!wxml.contains("bindtap"));
    assert!(!page.contains("await()"));
    assert!(has_diagnostic(&wechat, "EVENT_EXPRESSION_UNSUPPORTED"));
}

#[test]
fn preserves_literal_mustache_without_creating_target_bindings() {
    let html = r#"<p data-label="{{ customer }}">{{ customer }}</p>"#;

    let vue = translate_html(html, Target::Vue, None);
    let vue_component = &file(&vue, "TranslatedComponent.vue").content;
    assert!(vue_component.contains("v-text=\"&quot;{{ customer }}&quot;\""));
    assert!(vue_component.contains("data-label=\"&#123;&#123; customer &#125;&#125;\""));
    assert!(!vue_component.contains(">{{ customer }}</p>"));
    assert!(has_diagnostic(&vue, "TEMPLATE_BINDING_ESCAPED"));

    let wechat = translate_html(html, Target::WechatMiniProgram, None);
    let wxml = &file(&wechat, "translated-component.wxml").content;
    assert!(wxml.contains("data-label=\"&#123;&#123; customer &#125;&#125;\""));
    assert!(wxml.contains(">&#123;&#123; customer &#125;&#125;</view>"));
    assert!(has_diagnostic(&wechat, "TEMPLATE_BINDING_ESCAPED"));
}

#[test]
fn omits_invalid_react_inline_style_properties() {
    let result = translate_html(
        r#"<div style="foo bar: x; font-size: 12px"></div>"#,
        Target::React,
        None,
    );

    let component = &file(&result, "TranslatedComponent.tsx").content;
    assert!(component.contains("style={{ fontSize: \"12px\" }}"));
    assert!(!component.contains("foo bar"));
    assert!(has_diagnostic(&result, "REACT_INLINE_STYLE_OMITTED"));
}

#[test]
fn rejects_empty_html() {
    let result = translate(TranslationRequest {
        html: "  \n\t".to_string(),
        target: Target::Vue,
        component_name: None,
    });

    assert!(result.is_err());
}
