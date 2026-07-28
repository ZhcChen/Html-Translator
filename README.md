# Html Translator

`Html Translator` 是一个 Rust 多模块仓库，用于将受限的静态 HTML 转换为 Vue 3、React TSX 或原生微信小程序文件包，并通过 MCP stdio 提供给本地 AI agent 调用。

当前版本定位为迁移助手：它会生成可维护的静态页面初稿和结构化诊断，而不会声称可无损迁移浏览器运行时逻辑。

## 工作区

```text
crates/
  html-translator-core/  # 与协议无关的解析、规则、生成和诊断
  html-translator-mcp/   # 基于 rmcp 的本地 stdio MCP 服务
```

`html-translator-core` 不依赖 MCP、Tokio 或 stdio，因此后续可以安全地增加 CLI、HTTP 或其他调用适配层。

## 支持的输出

| `target` | 文件包 |
| --- | --- |
| `vue` | `<PascalName>.vue` |
| `react` | `<PascalName>.tsx`、`<PascalName>.css` |
| `wechat_mini_program` | `<kebab-name>.wxml`、`.wxss`、`.js`、`.json` |

初始映射覆盖常用容器、文本、图片、链接、按钮、输入框和文本域。微信小程序端会将常见容器映射为 `view`，纯行内文本映射为 `text`，图片映射为 `image`，内部页面链接映射为 `navigator`。

## 边界与诊断

- `<style>` 被提取为目标样式文件或 Vue SFC 样式块；Vue 和 React 默认保留 CSS。
- 微信小程序将 CSS 原样输出为 WXSS，并产生兼容性审查诊断；本阶段不自动将 `px` 换算为 `rpx`。
- 完整 HTML 文档只转换 `body` 中的可见结构；`head` 中只提取 `<style>`。
- `<script>` 和外链样式会被省略并返回诊断；目标平台不支持的标签会按目标规则降级或返回诊断。
- 仅 `onclick="handleClick"` 这类非保留字的函数标识符事件会映射为目标事件绑定，并生成同名空 handler stub；任意表达式、保留字或源脚本不会执行或复制。
- 字面 `{{ ... }}` 在 Vue 与微信小程序目标中会被静态化并返回诊断，避免被错误解释为数据绑定。
- React 内联样式只生成安全的对象键；无法安全映射的样式声明会被省略并返回诊断。
- 每次调用都会返回 `files` 和 `diagnostics`，供调用方或 AI agent 继续完成手工迁移。

## 构建与验证

需要 Rust `1.88` 或更高版本。

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## 作为 MCP 服务运行

先构建 release 二进制：

```bash
cargo build --release --package html-translator-mcp
```

二进制路径为：

```text
target/release/html-translator-mcp
```

MCP 客户端配置示例：

```json
{
  "mcpServers": {
    "html-translator": {
      "command": "/absolute/path/to/Html-Translator/target/release/html-translator-mcp"
    }
  }
}
```

开发阶段也可由 Cargo 启动：

```json
{
  "mcpServers": {
    "html-translator": {
      "command": "cargo",
      "args": ["run", "--quiet", "--package", "html-translator-mcp"],
      "cwd": "/absolute/path/to/Html-Translator"
    }
  }
}
```

服务只通过 stdin/stdout 交换换行分隔的 MCP JSON-RPC 消息。stdout 不输出日志，便于标准 MCP 客户端安全解析。

## MCP 工具

工具名：`translate_html`

```json
{
  "html": "<section class=\"card\"><span>Hello</span></section>",
  "target": "wechat_mini_program",
  "componentName": "welcome-card"
}
```

工具返回 MCP `structuredContent`，内容包含：

```json
{
  "files": [
    { "path": "welcome-card.wxml", "content": "..." }
  ],
  "diagnostics": [
    {
      "severity": "warning",
      "code": "WECHAT_CSS_REVIEW_REQUIRED",
      "message": "..."
    }
  ]
}
```
