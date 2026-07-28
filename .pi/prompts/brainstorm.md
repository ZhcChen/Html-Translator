---
description: 需求不清或方案分叉时，先做前置收敛并写入 docs/brainstorms/
argument-hint: "[主题或问题]"
---
请把这次请求当作一次 `brainstorm` 工作流入口，先做需求澄清和方案收敛，而不是直接开始实现。

用户输入：${ARGUMENTS:-使用当前对话上下文}

执行要求：
- 仅在需求不清、范围未定、方案分叉或未知项较多时使用；如果需求已经清晰，明确说明可直接进入 `/plan`
- 优先复用 `docs/brainstorms/` 下已有正式文档；没有合适文档时，参考 `docs/brainstorms/TEMPLATE.md`，新建具体文件，例如 `docs/brainstorms/YYYY-MM-DD-short-name.md`
- 不把正式内容直接写进 `docs/brainstorms/TEMPLATE.md`
- 文档统一使用简体中文，并在涉及文件时使用仓库相对路径
- 至少覆盖：背景、目标、约束、备选方案、待确认问题、当前倾向、下一步
- 如果仍存在关键未知项，先把未知项问清或显式记录假设，不要直接跳到实现
- 结束时明确给出下一步：进入 `docs/plans/`、继续澄清，还是暂停
