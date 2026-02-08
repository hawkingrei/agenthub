# AgentHub 项目约定

本文件用于记录 AgentHub 的目标、范围、架构决策与开发约定，作为后续实现与演进的基准。

## 1. 项目目标

AgentHub 是一个远程控制 AI Agent 的工具，支持在网页端开启、管理和交互 Agent。Agent 在指定路径下运行，网页端可看到输出并进行互动。Agent 完成工作后可主动推送消息。除非用户主动关闭 session，否则即使网页关闭也要保留 Agent 存在。

## 2. 范围与功能清单（MVP）

- Agent 生命周期管理：创建、启动、停止、重连、销毁
- 实时输出与交互：默认 HTTP/轮询或 SSE；WebSocket 作为可选项
- 管理后台：Agent 列表、状态、日志、会话详情
- 认证与安全：Passkey 登录（WebAuthn）、基础访问控制
- 持久化：SQLite 存储会话、Agent 配置、审计记录
- 通知：Agent 完成后发送站内通知（后续可扩展 webhook/email）
  - 使用浏览器 Push API（后续扩展 Webhook）

## 3. 技术与架构约束

- 后端：Rust（单进程服务）
- 前端：主流 TS 框架（默认采用 React + Vite SPA），静态资源嵌入 Rust 服务
- 数据库：SQLite
- 部署：单体二进制，无独立前端部署
- Agent 运行：在用户指定路径下启动子进程；网页关闭不影响 Agent 存活

## 4. 关键架构决策

1) 前端采用 SPA 静态构建（Vite），由 Rust 作为静态文件服务器与 API 服务。
2) Agent 输出默认走非 WS 通道；WS 作为可选增强（未来 bash sandbox 串流）。
3) Agent 生命周期由后端进程管理；会话与运行状态持久化到 SQLite。
4) 登录采用 Passkey（WebAuthn）；服务端需保存 credential 与挑战数据。
5) ACP（Agent Control Protocol）用于 agent 输出的结构化渲染，必须保留历史记录。

## 5. 目录规划（可调整）

```
agenthub/
  src/
    main.rs
    api/
    agent/
    auth/
    db/
    ws/
  web/
    package.json
    src/
    dist/                # build 输出
  migrations/
  AGENTS.md
```

## 6. 安全与可靠性原则

- 默认最小权限：Agent 仅能访问指定路径
- 严格输入校验：所有 API 参数需校验
- 会话持久化：断线可重连，不自动关闭
- 审计记录：关键操作写入数据库日志
  - 设备登录审计、设备撤销、路径删除等必须留痕

## 7. 测试与验证（初始建议）

- Passkey 注册/登录流程
- ACP 渲染与历史回放
- WS 断线重连与消息完整性（可选项）
- Agent 长时间运行与资源清理
- SQLite 事务一致性与并发访问

## 8. 后续可扩展方向

- 通知渠道：Web Push / Webhook / Email
- 多用户/多租户
- Agent 插件与执行沙箱
  - Bash sandbox 串流（启用 WS）

## 9. 需求补充（最新上下文）

- Agents 页面：
  - 顶部表单创建任务，支持选择 workdir 与 worktree 策略
  - 下方展示正在执行与历史任务卡片
  - 卡片提供“查看执行情况”，使用 ACP 渲染（类似 Xcode 运行视图）
- Admin 配置：
  - 可设置每个 agent 是否开启“代码模式”
- Join/登录：
  - 登录仅需 username + password；Display Name 仅用于注册/Bootstrap
- 配置方式：
  - 使用配置文件而非环境变量
- ACP：
  - 必须保留历史记录并可回放

## 10. TODO

- ACP stdio client：与 agenthub-codex-acp 对接并完善权限交互
- ACP HTTP3 gateway（对外公开地址）
- ACP 权限交互优化：弹窗式确认
- ACP 权限事件推送：WebSocket 替代轮询
- Worktree 策略实现与 UI
- Admin 端 agent code mode 开关
- 统一配置文件加载与校验
- A2A 多智能体并行与排序：全局有序事件流（建议 DB 自增序列或单点发号器）

## 11. Documentation And Context Notes

- Every change must be documented.
- Add a TODO entry in `docs/todo.md` for follow-up or verification items.
- Add a feature note under `docs/features/` describing background, scope, key decisions, and validation.
- Feature notes should use `YYYY-MM-DD-topic.md` naming for easy lookup.

## 12. 变更记录

### 2026-02-05

- ACP 会话视图改为“底部堆叠 + 可滚动”，并在新消息到来时自动吸底（用户手动上滚后不强制吸底）
- ACP 容器高度约束与滚动行为修正（`output-body`/`acp`/`acp-conversation` 统一使用 flex 约束）
- ACP 思考块折叠规则调整：仅在首次 agent_message 出现后折叠
- 用户输入消息去重：引入 `message_id`，避免 WS 与 HTTP 双通道导致的重复展示
- 会话过滤规则放宽：允许 `session_id` 缺失的消息进入当前会话视图
- 移动端体验优化：Agents 面板改为覆盖式抽屉并增加遮罩；输入区与按钮尺寸缩小
- Agents 折叠按钮统一放到 Output 标题旁，避免折叠后无法恢复
- UI 空白收紧：`acp-head` 顶部空白减少并改为 `align-items: flex-start`
- 新增样式守卫测试：`tests/web_assets.rs` 确认 ACP 容器关键样式（避免回归）
- 输出历史改为“上滑加载”：滚动接近顶部时从数据库增量拉取更早记录（移除固定 Recent 标签）
- Conversation 视图改为瀑布流顺序展示：`agent_thought` 作为独立 `agent_thinking` 气泡，按 `seq` 插入在用户与 AI 回复之间
- `agent_thinking` 在思考结束后默认折叠，思考进行中保持展开
- 本地输入事件补齐全局 `seq` 并参与排序，避免多条用户输入被错误合并或乱序
- A2A 需求补充：多机器人发言的全局顺序必须严格可重放
- Output 滚动策略调整：`output-body` 固定高度并禁用自身滚动，ACP/Terminal 内部各自滚动显示
- 输出历史自动补齐：当当前会话消息不足时，自动向上翻页拉取更早记录
- 默认激活运行中的 Agent：首次进入页面若未选中，自动选择第一个运行中的 Agent（否则选择列表首个）
- 输入区固定在 Output 底部（`.input.docked` 使用 `margin-top: auto`）
