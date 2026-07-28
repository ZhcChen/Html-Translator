---
description: 把关键决策、坑点或可复用经验沉淀到 docs/solutions/
argument-hint: "[主题、问题或关联计划]"
---
请把这次请求当作一次 `compound` 工作流入口，把值得复用的经验、决策或排查路径沉淀到 `docs/solutions/`。

用户输入：${ARGUMENTS:-使用当前对话上下文}

执行要求：
- 只有在出现关键决策、复发坑点、有效排查路径、稳定模式或值得复用的经验时，才写沉淀
- 优先复用已有 `docs/solutions/` 正式文档；没有合适文档时，参考 `docs/solutions/TEMPLATE.md` 新建具体文件，例如 `docs/solutions/YYYY-MM-DD-short-name.md`
- 不把正式内容直接写进 `docs/solutions/TEMPLATE.md`
- 文档统一使用简体中文，并在涉及文件时使用仓库相对路径
- 至少覆盖：摘要、背景、关键结论、可复用建议、验证 / 证据、后续事项
- 只沉淀真正能复用的内容，不要把一次性执行日志原样搬进去
- 结束时说明这份沉淀解决了什么复用问题，以及后续谁在什么场景下应该优先查看它
