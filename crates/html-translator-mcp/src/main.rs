#![forbid(unsafe_code)]

#[tokio::main]
async fn main() {
    if let Err(error) = html_translator_mcp::run_stdio().await {
        eprintln!("html-translator-mcp failed: {error}");
        std::process::exit(1);
    }
}
