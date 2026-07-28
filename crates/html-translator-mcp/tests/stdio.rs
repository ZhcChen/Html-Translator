use std::{process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    time::timeout,
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn stdio_server_completes_initialize_discovery_and_translation() -> TestResult {
    let binary = env!("CARGO_BIN_EXE_html-translator-mcp");
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let mut stdin = child.stdin.take().expect("child should expose stdin");
    let stdout = child.stdout.take().expect("child should expose stdout");
    let mut stdout = BufReader::new(stdout);

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "stdio-test", "version": "0.1.0" }
            }
        }),
    )
    .await?;
    let initialize = read_response(&mut stdout, 1).await?;
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "html-translator"
    );
    assert!(initialize["result"]["capabilities"]["tools"].is_object());

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )
    .await?;

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await?;
    let tools = read_response(&mut stdout, 2).await?;
    let tool = tools["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "translate_html"))
        .expect("translate_html should be discoverable");
    assert!(tool["inputSchema"]["properties"]["html"].is_object());
    assert!(tool["inputSchema"]["properties"]["target"].is_object());
    assert!(tool["inputSchema"]["properties"]["componentName"].is_object());
    assert!(tool["outputSchema"].is_object());

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "translate_html",
                "arguments": {
                    "html": "<section class=\"card\"><span>Hello</span></section>",
                    "target": "wechat_mini_program",
                    "componentName": "mcp-card"
                }
            }
        }),
    )
    .await?;
    let response = read_response(&mut stdout, 3).await?;
    assert_eq!(response["result"]["isError"], false);
    let structured_content = &response["result"]["structuredContent"];
    assert!(structured_content["files"].is_array());
    assert!(structured_content["diagnostics"].is_array());
    assert!(
        structured_content["files"]
            .as_array()
            .expect("files should be an array")
            .iter()
            .any(|file| file["path"] == "mcp-card.wxml")
    );
    let text_content = response["result"]["content"]
        .as_array()
        .and_then(|content| content.first())
        .and_then(|content| content["text"].as_str())
        .expect("tool response should include JSON text content");
    let text_result: Value = serde_json::from_str(text_content)?;
    assert_eq!(&text_result, structured_content);

    drop(stdin);
    assert_stdout_is_valid_jsonrpc_until_eof(&mut stdout).await?;
    drop(stdout);
    let status = timeout(RESPONSE_TIMEOUT, child.wait()).await??;
    assert!(status.success(), "MCP server should shut down cleanly");

    let mut stderr = child.stderr.take().expect("child should expose stderr");
    let mut stderr_output = Vec::new();
    stderr.read_to_end(&mut stderr_output).await?;
    assert!(
        stderr_output.is_empty(),
        "normal MCP operation should not log to stderr: {}",
        String::from_utf8_lossy(&stderr_output)
    );

    Ok(())
}

async fn send_json<W>(writer: &mut W, message: &Value) -> TestResult
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(serde_json::to_string(message)?.as_bytes())
        .await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn read_response<R>(
    reader: &mut BufReader<R>,
    expected_id: u64,
) -> Result<Value, Box<dyn std::error::Error>>
where
    R: AsyncRead + Unpin,
{
    loop {
        let mut line = String::new();
        let bytes = timeout(RESPONSE_TIMEOUT, reader.read_line(&mut line)).await??;
        if bytes == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("server closed stdout before response {expected_id}"),
            )
            .into());
        }

        let message: Value = serde_json::from_str(line.trim_end())?;
        validate_jsonrpc_message(&message)?;
        if message["id"] == expected_id {
            return Ok(message);
        }
    }
}

async fn assert_stdout_is_valid_jsonrpc_until_eof<R>(reader: &mut BufReader<R>) -> TestResult
where
    R: AsyncRead + Unpin,
{
    loop {
        let mut line = String::new();
        let bytes = timeout(RESPONSE_TIMEOUT, reader.read_line(&mut line)).await??;
        if bytes == 0 {
            return Ok(());
        }

        let message: Value = serde_json::from_str(line.trim_end())?;
        validate_jsonrpc_message(&message)?;
    }
}

fn validate_jsonrpc_message(message: &Value) -> TestResult {
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stdout contains JSON that is not a JSON-RPC 2.0 message",
        )
        .into());
    }

    let is_notification_or_request = message.get("method").and_then(Value::as_str).is_some();
    let is_response = message.get("id").is_some()
        && (message.get("result").is_some() || message.get("error").is_some());
    if !is_notification_or_request && !is_response {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stdout contains an incomplete JSON-RPC message",
        )
        .into());
    }

    Ok(())
}

type TestResult = Result<(), Box<dyn std::error::Error>>;
