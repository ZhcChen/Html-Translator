# Rust MCP stdio 与 HTML 规范化经验

## 标题信息

- 主题：将 Rust 转换核心接入 MCP stdio 时的协议与解析边界
- 日期：2026-07-28
- 关联计划：`docs/plans/2026-07-28-rust-core-mcp-stdio.md`

## 摘要

首个 Rust 转换核心通过 `rmcp` 提供本地 MCP stdio 服务。实现中确认：仅把 JSON 序列化到 MCP 的文本内容不足以支撑 AI agent 稳定调用，必须暴露 JSON Schema 并通过 `structuredContent` 返回结果；HTML 解析器会规范化片段结构，`<script>` 可能被移入 `head`，因此不能只在 body 遍历中处理不支持节点。

## 背景

项目的调用方包括本地 AI agent。它们需要发现工具参数、可靠读取生成文件与诊断，并且不能因 stdout 日志或隐式 JSON 字符串解析而失效。

输入 HTML 同时可能是片段或完整文档。基于浏览器兼容 HTML 解析器的实现会自动补齐 `html`、`head`、`body`，并根据标签类型重排节点；这与直接扫描输入字符串的结果不同。

## 关键结论

- 转换核心保持为无 MCP、Tokio、stdio 依赖的 Rust library；MCP crate 只负责协议请求/响应映射。
- MCP 工具的输入和输出专用类型应派生 `JsonSchema`，并使用 `rmcp::handler::server::wrapper::Json<T>` 返回，以同时提供 `outputSchema` 和 `structuredContent`。
- stdio 服务 stdout 只能输出换行分隔的 JSON-RPC；任何日志必须写入 stderr。
- stdio 验证不能只检查进程退出码。必须完成 `initialize`、`notifications/initialized`、`tools/list`、`tools/call`，并逐行解析 stdout。
- HTML 规范化需要统一处理 `head` 和 `body`：`head` 仅提取 `<style>`，`<script>` 和外链样式必须诊断而非静默遗漏。
- 对函数标识符事件生成空 handler stub 是一种明确的降级策略，必须附带诊断；任意脚本表达式、JavaScript/TypeScript 保留字都不能复制为 handler 名。
- Vue/WXML 对 `{{ ... }}` 有模板绑定语义。静态 HTML 的字面 Mustache 必须按目标语法静态化并诊断，属性值也不能遗漏。
- React 内联 style 的键必须验证为可安全映射的 JavaScript 对象键；无效 CSS 声明应诊断并省略，不能拼出非法 TSX。

## 可复用建议

- 为每个目标固定文件包契约，并通过 fixture 断言完整路径集合，而不是只断言一段生成文本。
- 协议适配层不要泄漏到核心模型；MCP 输出类型可单独映射核心结果以避免 SDK 依赖扩散。
- 为代码生成器加入保留字 handler、字面 Mustache、非法 style 属性名的回归 fixture，避免生成目标代码在编译阶段失效。
- 使用真实子进程测试 stdio transport，避免内存 transport 掩盖 stdout、换行或进程关闭问题。
- 将 CSS 先作为保守透传并诊断；在引入 CSS AST 后再逐步增加 WXSS 选择器和属性转换。
- `rmcp` 当前使用 beta 版本，升级时优先运行 schema 与 stdio 集成测试，确认协议行为未漂移。

## 验证 / 证据

- 命令：`cargo fmt --check`
- 命令：`cargo clippy --workspace --all-targets -- -D warnings`
- 命令：`cargo test --workspace`
- 命令：`cargo build --release --package html-translator-mcp`
- 文件：`crates/html-translator-mcp/tests/stdio.rs`
- 观察：release 二进制完成初始化、工具发现和 `translate_html` 调用，返回四个微信小程序文件与 `structuredContent`。

## 后续事项

- 使用 CSS AST 增加 WXSS 选择器、属性和值的兼容性检查。
- 扩展标签、表单、资源和样式规则时，持续添加跨目标 fixture。
- 在 MCP 协议或 `rmcp` 升级时复查 schema、stdio 行协议和错误结果。
