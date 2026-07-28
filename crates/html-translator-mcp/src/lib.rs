#![forbid(unsafe_code)]

use html_translator_core::{
    Diagnostic, DiagnosticSeverity, GeneratedFile, Target, TranslationRequest, TranslationResult,
    translate,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct HtmlTranslatorMcp {
    tool_router: ToolRouter<Self>,
}

impl HtmlTranslatorMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for HtmlTranslatorMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl HtmlTranslatorMcp {
    /// 将静态 HTML 转换为指定前端目标的文件包。
    #[tool(
        name = "translate_html",
        description = "将静态 HTML 转换为 Vue、React 或微信小程序文件包，并返回诊断信息。"
    )]
    pub async fn translate_html(
        &self,
        Parameters(input): Parameters<TranslateHtmlInput>,
    ) -> Result<Json<TranslateHtmlOutput>, String> {
        let result = translate(TranslationRequest {
            html: input.html,
            target: input.target.into(),
            component_name: input.component_name,
        })
        .map_err(|error| error.to_string())?;

        Ok(Json(result.into()))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for HtmlTranslatorMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("html-translator", env!("CARGO_PKG_VERSION")),
        )
    }
}

pub async fn run_stdio() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = HtmlTranslatorMcp::new()
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranslateHtmlInput {
    /// 要转换的 HTML 片段或完整 HTML 文档。
    pub html: String,
    /// 目标代码平台。
    pub target: TargetInput,
    /// 可选的组件名；会生成安全的目标文件名。
    #[serde(default)]
    pub component_name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetInput {
    Vue,
    React,
    WechatMiniProgram,
}

impl From<TargetInput> for Target {
    fn from(value: TargetInput) -> Self {
        match value {
            TargetInput::Vue => Self::Vue,
            TargetInput::React => Self::React,
            TargetInput::WechatMiniProgram => Self::WechatMiniProgram,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranslateHtmlOutput {
    pub files: Vec<GeneratedFileOutput>,
    pub diagnostics: Vec<DiagnosticOutput>,
}

impl From<TranslationResult> for TranslateHtmlOutput {
    fn from(value: TranslationResult) -> Self {
        Self {
            files: value.files.into_iter().map(Into::into).collect(),
            diagnostics: value.diagnostics.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFileOutput {
    pub path: String,
    pub content: String,
}

impl From<GeneratedFile> for GeneratedFileOutput {
    fn from(value: GeneratedFile) -> Self {
        Self {
            path: value.path,
            content: value.content,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticOutput {
    pub severity: DiagnosticSeverityOutput,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl From<Diagnostic> for DiagnosticOutput {
    fn from(value: Diagnostic) -> Self {
        Self {
            severity: value.severity.into(),
            code: value.code,
            message: value.message,
            context: value.context,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverityOutput {
    Info,
    Warning,
    Error,
}

impl From<DiagnosticSeverity> for DiagnosticSeverityOutput {
    fn from(value: DiagnosticSeverity) -> Self {
        match value {
            DiagnosticSeverity::Info => Self::Info,
            DiagnosticSeverity::Warning => Self::Warning,
            DiagnosticSeverity::Error => Self::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_a_structured_translation_tool() {
        let server = HtmlTranslatorMcp::new();
        let tool = server
            .tool_router
            .list_all()
            .into_iter()
            .find(|tool| tool.name == "translate_html")
            .expect("translate_html tool should be registered");

        let properties = tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("tool should expose input properties");
        assert!(properties.contains_key("html"));
        assert!(properties.contains_key("target"));
        assert!(properties.contains_key("componentName"));
        assert!(tool.output_schema.is_some());
    }

    #[tokio::test]
    async fn returns_files_and_diagnostics_as_structured_output() {
        let server = HtmlTranslatorMcp::new();
        let Json(result) = server
            .translate_html(Parameters(TranslateHtmlInput {
                html: "<div class=\"card\">Hello</div>".to_string(),
                target: TargetInput::Vue,
                component_name: Some("mcp-card".to_string()),
            }))
            .await
            .expect("tool call should succeed");

        assert_eq!(result.files[0].path, "McpCard.vue");
        assert!(
            result.files[0]
                .content
                .contains("<div class=\"card\">Hello</div>")
        );
    }
}
