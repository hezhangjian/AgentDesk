# 企业 Agent 接入协议 v1

AgentDesk 为每轮消息启动一次配置的可执行文件，将项目路径设为工作目录。启动参数是 JSON 字符串数组，各项独立传递；不经过 shell。适配程序可在内部调用企业 CLI 或 HTTP 服务。

## 请求

stdin 接收一个 UTF-8 JSON 对象，后跟换行与 EOF：

```json
{
  "directory": "/absolute/path/to/project",
  "model": "",
  "prompt": "请梳理项目结构",
  "session_id": null
}
```

`session_id` 首次为 null，续接时为上次 `session` 事件返回的 ID。适配程序负责将它映射到真实的远端会话，保留上下文。模型为空表示使用默认模型。认证通过 Agent 自身配置或继承的环境变量处理。

## 输出事件

stdout 只输出 JSON Lines，每行一个事件并及时 flush。诊断日志写入 stderr。每条事件最多 1 MiB，超限或无效 JSON 会使该轮失败。推荐工具输出先摘要，完整内容放在 Agent 自己的日志里。

请尽早返回会话 ID。没有会话 ID 的已有回复不能在桌面续接，以免悄悄丢失上下文。

```jsonl
{"type":"session","id":"private-session-123"}
{"type":"text","id":"reply-unique-id","text":"正在检查项目。","delta":true}
{"type":"tool","id":"call-unique-id","name":"读取 README","details":"读取成功，内容…","failed":false}
{"type":"text","id":"reply-final-unique-id","text":"项目包含以下模块…","delta":false}
```

| 事件 | 字段与语义 |
| --- | --- |
| `error` | `message`：失败原因；这轮显示为失败，执行程序被终止 |
| `notice` | `message`：诊断信息，不是 Agent 回复 |
| `session` | `id`：持久化的远端会话 ID |
| `text` | `id`、`text`、可选 `delta`：同一 ID 在 `delta: true` 时追加，默认替换 |
| `tool` | `id`、`name`、`details`、可选 `failed`：同一 ID 更新工具卡片，默认未失败 |

文本 ID 和工具 ID 必须在整个会话内唯一；只有更新同一条消息或同一次调用时才复用。若需要文本出现在工具卡片之后，应使用一个新的文本 ID。工具事件表示一次调用的结果；`failed` 表示工具失败，不必然意味着整轮任务失败。

程序退出代表这轮结束。退出码 0 且至少有一条文本或工具事件时为完成；非零退出、`error` 事件、无有效内容或输出协议错误时为失败。`finished` 是桌面内部事件，不属于外部协议。

点「停止」会终止适配进程；在 Unix 上也会终止它启动的同一进程组中的工具。适配远端服务时应自行实现客户端断开后的服务端取消，否则停止本地进程不保证远端任务停止。第一版没有权限请求与交互问题的事件类型，适配程序应自行遵守企业策略并明确报告不能执行的操作。

## 无模型的手动测试

仓库提供 `examples/stdio-agent.py`，它只是带明确标签的协议测试程序，不包含 AI，也不执行用户任务。

在 Agent 设置中选择「企业私有 Agent」：

- 可执行文件：`python3` 的绝对路径（可用 `command -v python3` 查看）。
- 启动参数：`["/absolute/path/to/AgentDesk/examples/stdio-agent.py"]`。
- 模型：留空。

保存后新建会话，发送任意文字。应依次看到文本、工具卡片及追加内容。再次发送会沿用同一个 `fixture-…` 会话 ID；重启应用后选择最近会话应能继续。测试完成后切回 OpenCode 并新建会话。
