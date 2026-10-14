# AgentDesk

一个轻量的原生 Agent 桌面，基于 **Rust + GPUI**。AgentDesk 管理项目和会话，复用 OpenCode 或企业自己的 CodeAgent 执行任务。

## 第一版

- **新建会话**：选择项目，输入任务，Enter 发送，Shift + Enter 换行。
- **项目列表**：通过原生文件夹选择器添加项目，按名称排序，自动去重。
- **最近会话**：按最近活动排序，保留历史、工作目录、Agent 配置和远端会话 ID。
- **聊天工作区**：Markdown 回复、可展开的工具调用详情、复制回复、运行状态、错误和停止按钮。不同会话可以同时运行。

所有项目与聊天记录保存在本机，不自建模型、工具系统或 Agent 循环。

## 运行

需要 Rust 工具链，以及 GPUI 对应平台的构建依赖。当前在 macOS 上验证；macOS 需要 Xcode Command Line Tools。

```sh
cargo run --locked
```

首次打开即可输入任务；尚未选择项目时，发送会打开文件夹选择器，选定后继续发送。也可以先点击「添加项目文件夹」。取消文件夹选择会保留输入内容。默认使用 `opencode`，可在左下角「Agent 设置」配置可执行文件和模型。

### OpenCode

先安装 [OpenCode](https://opencode.ai/docs/)，并在终端配置登录和模型：

```sh
opencode auth login
```

应用通过 `opencode run --format json --dir <project>` 执行，使用 `--session` 续接会话。消息通过标准输入传递，不拼接 shell 命令。已核对 OpenCode 1.18.33 的 CLI 和事件格式；详见 [官方 CLI 文档](https://opencode.ai/docs/cli/)。

OpenCode 的这个接口会在文本段落或工具执行结束后发出事件，界面按事件更新，并非逐 token 输出。登录、MCP、模型以及工具权限沿用 OpenCode 自身的配置。AgentDesk 不附加 `--auto`；需要交互审批的操作会被 OpenCode 的非交互模式拒绝，相关结果会展示在会话中。第一版尚未实现桌面权限审批和交互问答。

从 Finder 启动时会额外查找 `/opt/homebrew/bin`、`/usr/local/bin` 和 `~/.opencode/bin`。其他安装方式可以填写可执行文件的绝对路径。

### 企业私有 CodeAgent

「Agent 设置 → 企业私有 Agent」支持启动一个本地适配程序，通过 stdin / JSON Lines 对接现有企业 Agent。远端 HTTP、内部鉴权等由这个程序处理，桌面无需知道服务端细节。

完整协议与手动验证方式见 [Agent 接入协议](docs/agent-protocol.md)。任意现有 CLI 若不支持该协议，需要一个薄适配程序，不能直接填入后就自动兼容。

设置只影响新会话。已有会话固定使用创建时的工作目录和 Agent 配置，避免将一个 Agent 的会话 ID 传给另一个 Agent。

## macOS 应用包

```sh
./scripts/bundle-macos.sh debug
open target/debug/AgentDesk.app
```

发布构建使用 `./scripts/bundle-macos.sh release`。生成的是本地开发应用包，尚未配置发行签名、公证或自动更新。

## 本地数据

默认保存到系统应用数据目录下的 `AgentDesk/workspace.json`，macOS 对应 `~/Library/Application Support/AgentDesk/workspace.json`。通过临时文件与重命名保存；Unix 上文件权限为 `0600`。损坏或版本不支持的数据不会被静默覆盖。应用重启后，未完成的任务显示为中断，可以重新发送消息继续。

可通过环境变量使用独立数据目录：

```sh
AGENTDESK_DATA_DIR=/tmp/agentdesk-dev cargo run --locked
```

不要同时打开多个使用同一数据目录的实例。当前没有跨进程写入协调。聊天记录和工具输出以明文保存在本机；不在 AgentDesk 设置中保存服务端密钥。

## 开发与验证

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

测试覆盖事件解析、stdin 传递、会话续接、取消及子进程清理、异常输出、数据持久化和恢复。Unix 下取消会终止该任务的进程组；Windows 目前只终止主进程，尚未验证桌面和进程树清理。

可在专用空目录执行一次真实 OpenCode 冒烟测试（使用其当前模型配置）：

```sh
mkdir -p /tmp/agentdesk-smoke
cargo run --example agent-smoke -- /tmp/agentdesk-smoke '只回复 AgentDesk connected，不调用工具。'
```

再次运行时，可在提示词后传入上次输出的 `session` 事件 ID 来验证续接。

## 代码结构

| 目录 | 职责 |
| --- | --- |
| `src/agent/` | Agent 适配、进程生命周期、统一事件 |
| `src/model.rs` | 项目、会话和事件归并 |
| `src/storage.rs` | 本地数据读取与原子替换 |
| `src/ui/` | 会话、项目导航与设置界面 |

首版暂不包含文件编辑器、Git diff、插件市场、搜索、会话删除、任务调度和远端工作区同步。
