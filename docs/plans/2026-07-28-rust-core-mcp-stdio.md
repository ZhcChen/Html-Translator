# Rust 转换核心与 MCP stdio 服务初始化计划

## 标题信息

- 任务：初始化多模块仓库，建立 Rust HTML 转换核心和 MCP stdio 服务
- 状态：已完成
- 负责人：待定
- 日期：2026-07-28

## 目标

完成后，仓库应具备以下可验证能力：

- Git 使用 `main` 分支并配置 `origin` 为 `git@github.com:ZhcChen/Html-Translator.git`。
- Cargo workspace 至少包含独立的转换核心 crate 与 MCP 服务 crate。
- 核心库可将受限的静态 HTML 转换为 Vue 3、React TSX 或微信小程序 WXML/WXSS 文件包，并返回结构化诊断。
- MCP 服务通过标准输入输出运行，向 AI agent 暴露 `translate_html` 工具。
- Rust 测试、格式化、静态检查通过；首个提交推送至远端。

## 范围

本次实现以下 MVP：

- 输入 HTML 片段或完整文档中的静态结构、文本、常见属性、内联样式和 `<style>`。
- 支持 Vue 3、React TSX、微信小程序三种目标。
- 支持第一批高频标签：容器、文本、图片、链接、按钮、输入框和文本域。
- Vue/React 保留大部分 HTML/CSS 结构，只处理必要的语法和属性差异。
- 小程序按集中规则映射容器到 `view`、行内纯文本到 `text`、图片到 `image`、内部链接到 `navigator`，并输出 WXSS。
- 对不安全或不等价的脚本、事件表达式、外部链接、未支持标签和高风险样式输出诊断；仅将函数标识符形式的事件转换为目标事件绑定，并生成同名空 stub，绝不执行或复制源脚本。
- MCP 仅提供本地 stdio 通道，不开放网络端口。

## 非目标

- 不迁移任意 JavaScript、DOM API、状态管理、路由逻辑或第三方组件。
- 不承诺任意 CSS 在微信小程序端的视觉无损还原。
- 不实现网页编辑器、HTTP API、鉴权、持久化任务或云端服务。
- 不在本阶段实现 CSS `px` 到 `rpx` 的自动换算，保持原值并预留策略入口。
- 不重写远端历史，也不使用强制推送。

## 影响区域

- `Cargo.toml`：workspace 清单和共享依赖配置。
- `crates/html-translator-core/`：中间表示、映射规则、生成器和诊断。
- `crates/html-translator-mcp/`：基于 `rmcp` 的 MCP stdio 服务与工具定义。
- `README.md`：工作区说明、构建命令、CLI/MCP 启动方式和客户端配置示例。
- `.gitignore`：Rust 构建产物和本地工具文件。
- `docs/brainstorms/2026-07-28-html-to-multiplatform-frontend.md`：本计划的上游需求收敛。
- `docs/plans/2026-07-28-rust-core-mcp-stdio.md`：本执行计划。

## 接口约束

核心库公开稳定的请求和结果模型：

```text
TranslationRequest {
  html: String,
  target: Target,
  component_name: Option<String>
}

TranslationResult {
  files: Vec<GeneratedFile>,
  diagnostics: Vec<Diagnostic>
}
```

其中 `Target` 初始为 `Vue`、`React`、`WechatMiniProgram`。组件名若不符合目标语言标识符或安全文件名要求，必须回退到确定性的默认值，并附带诊断。

首期文件包契约固定如下：

| 目标 | 文件 | 规则 |
| --- | --- | --- |
| Vue | `<PascalName>.vue` | 生成 Vue 3 SFC，包含 `<template>`、按需 `<script setup>` handler stub 与可选 `<style>`。 |
| React | `<PascalName>.tsx`、`<PascalName>.css` | TSX 默认导出组件并导入同名样式文件；按需生成 handler stub。 |
| 微信小程序 | `<kebab-name>.wxml`、`.wxss`、`.js`、`.json` | 以页面文件包输出；`.js` 注册 `Page` 并包含按需 handler stub，`.json` 初始为空对象。 |

完整 HTML 文档只转换 `body` 中的可见结构；`head` 中仅提取 `<style>`，其他内容不进入目标组件并按规则生成诊断。

MCP 工具名固定为 `translate_html`，参数包含 `html`、`target` 和可选的 `componentName`。MCP 专用请求和结果类型必须派生 JSON Schema；工具使用 `rmcp::handler::server::wrapper::Json<T>` 返回，使结果同时出现在 `structuredContent` 与文本内容中。返回值必须同时包含生成文件和诊断，便于 AI agent 不依赖 stdout 人工解析。

MCP 的 stdio 实现使用 `rmcp` 官方 Rust SDK。服务端 stdout 只能输出合法 MCP JSON-RPC 消息，日志仅写入 stderr；集成测试必须逐行校验 stdout 均为 JSON-RPC。

## 实现思路

1. 建立 Cargo workspace，并将转换逻辑与协议适配层分离，避免 MCP 依赖进入核心库。
2. 将 HTML 解析为内部节点树，抽取 `<style>`，保留必要属性与文本；在规范化阶段生成不支持能力的诊断。
3. 建立目标规则表和共享渲染接口，分别生成 Vue、React 与微信小程序文件包。
4. 将核心请求映射为 MCP `translate_html` 工具，使用 stdio transport 提供给本地 AI agent。
5. 用 fixture 验证三个目标的关键标签映射和诊断，用 stdio 冒烟测试验证 MCP 初始化、工具发现与工具调用。

## 阶段拆分

### 阶段 1：工作区与核心契约

- 目标：创建可编译的 Rust workspace，确定核心库公开模型。
- 边界：只建立 crate、数据类型、错误与诊断模型，不实现完整转换。
- 验收重点：workspace 结构清晰，核心 crate 可被其他 crate 引用。

### 阶段 2：静态 HTML 转换核心

- 目标：完成 HTML 解析、映射规则和三个目标生成器。
- 边界：只处理静态结构和受限样式；脚本与复杂交互仅诊断。
- 验收重点：固定 HTML fixture 能生成预期的 Vue、React、WXML/WXSS 文件包。

### 阶段 3：MCP stdio 适配

- 目标：向标准 MCP 客户端公开转换工具。
- 边界：仅本地 stdio；不增加网络 transport 和资源管理功能。
- 验收重点：完成 `initialize`、`tools/list`、`tools/call` 交互，工具返回结构化转换结果。

### 阶段 4：文档、验证与发布

- 目标：补齐使用说明、MCP 配置示例、验证命令与初始 Git 提交。
- 边界：不引入网页 UI 或额外目标平台。
- 验收重点：所有检查通过，远端 `main` 可见首次提交。

## 执行单元

### 单元 1：初始化 Rust workspace

- 所属阶段：阶段 1
- 目标：创建根清单、两个 crate、忽略规则和最小文档。
- 涉及文件 / 模块：`Cargo.toml`、`.gitignore`、`crates/html-translator-core/`、`crates/html-translator-mcp/`、`README.md`。
- 前置依赖：Rust 与 Cargo 可用。
- 验证方式：`cargo check --workspace`。
- 完成标准：两个 crate 可独立命名、核心 crate 不依赖 MCP crate。

### 单元 2：实现核心类型、解析和规则表

- 所属阶段：阶段 2
- 目标：定义请求、结果、文件、诊断、HTML 节点和目标规则。
- 涉及文件 / 模块：`crates/html-translator-core/src/`。
- 前置依赖：单元 1。
- 验证方式：核心 crate 单元测试。
- 完成标准：可将 HTML 规范化为内部节点树，并能识别不支持输入。

### 单元 3：实现目标生成器和 fixture 测试

- 所属阶段：阶段 2
- 目标：生成 Vue、React 和微信小程序文件，并覆盖核心映射规则。
- 涉及文件 / 模块：`crates/html-translator-core/src/`、`crates/html-translator-core/tests/`。
- 前置依赖：单元 2。
- 验证方式：`cargo test -p html-translator-core`。
- 完成标准：同一 fixture 生成契约规定的精确文件集合和可读代码；函数标识符事件只生成空 stub 与警告，复杂事件不生成绑定；无损、降级和不支持项都有诊断。

### 单元 4：实现 MCP stdio 工具

- 所属阶段：阶段 3
- 目标：使用 `rmcp` 注册 `translate_html` 工具并通过 stdio 运行。
- 涉及文件 / 模块：`crates/html-translator-mcp/src/`。
- 前置依赖：单元 1、单元 3。
- 验证方式：MCP 工具定义测试，以及真实子进程 stdio 测试依次执行 `initialize`、`notifications/initialized`、`tools/list`、`tools/call`。
- 完成标准：初始化响应声明 tools capability；工具可发现且输入/输出 schema 完整；合法调用返回结构化转换结果；stdout 无非 JSON-RPC 内容。

### 单元 5：完成文档、复核和推送

- 所属阶段：阶段 4
- 目标：说明本地 MCP 配置，执行全量验证并推送提交。
- 涉及文件 / 模块：`README.md`、相关测试和 Git 元数据。
- 前置依赖：单元 1 至单元 4。
- 验证方式：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、stdio 冒烟调用、`git push -u origin main`。
- 完成标准：工作树仅含预期变更，远端分支已更新。

## `/goal` 建议作用域

- 当前建议把 `/goal` 绑定到阶段 1 至阶段 2 的连续单元，完成核心静态转换链路后再单独处理 MCP transport。
- 不建议将整个仓库初始化、转换器、MCP、文档和发布作为同一个 `/goal`。

## 验证方式

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- 使用真实子进程和 JSON-RPC stdin/stdout 客户端依次完成 `initialize`、`notifications/initialized`、`tools/list`、`tools/call`；断言服务信息、tools capability、工具 schema、`structuredContent` 与 stdout 纯净性。
- 检查 `git status --short`、提交内容和 `git ls-remote --heads origin main`。

## 执行结果

- 已初始化 `main` 分支并配置 `origin` 为 `git@github.com:ZhcChen/Html-Translator.git`。
- 已建立 `html-translator-core` 与 `html-translator-mcp` 两个 Rust crate，并保持核心 crate 不依赖 MCP/Tokio/stdin/stdout。
- 已实现 Vue 3、React TSX、微信小程序的静态 HTML 文件包生成、集中映射规则与结构化诊断。
- 已实现基于 `rmcp` 的 `translate_html` stdio MCP 工具，返回 JSON Schema 与 `structuredContent`。
- 已执行 `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、release 构建和 stdio JSON-RPC 冒烟验证。

## 风险 / 待确认问题

- HTML 到小程序的样式和交互并非总能等价；诊断必须优于静默输出。
- `rmcp` API 和 MCP 协议版本需要随依赖锁定并通过真实 stdio 交互验证。
- 远端仓库当前为空；首次推送前仍需检查访问权限和分支保护。
- Rust 核心只负责确定性转换，后续 AI agent 可在 MCP 结果基础上处理人工修正建议。

## 沉淀跟进

- 将首次遇到的 WXML/WXSS 映射限制、MCP stdio 调试步骤和 fixture 设计原则沉淀到 `docs/solutions/`。
