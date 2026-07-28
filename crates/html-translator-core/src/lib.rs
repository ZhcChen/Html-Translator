#![forbid(unsafe_code)]

//! Deterministic HTML-to-frontend-code translation primitives.

pub mod model;
mod parser;
mod render;
mod rules;

pub use model::{
    Diagnostic, DiagnosticSeverity, GeneratedFile, Target, TranslationError, TranslationRequest,
    TranslationResult,
};

/// Converts static HTML into files for one target platform.
pub fn translate(request: TranslationRequest) -> Result<TranslationResult, TranslationError> {
    if request.html.trim().is_empty() {
        return Err(TranslationError::EmptyHtml);
    }

    let mut diagnostics = Vec::new();
    let source = parser::parse_source(&request.html, &mut diagnostics);
    let files = render::render(
        &source,
        request.target,
        request.component_name.as_deref(),
        &mut diagnostics,
    );

    Ok(TranslationResult { files, diagnostics })
}
