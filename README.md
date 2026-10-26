# Ask Bridge · Gemini Edge Lite

面向个人中文校对的精简 fork，源自 [doggy8088/ask-bridge](https://github.com/doggy8088/ask-bridge)，保留 MIT 许可证。

Windows x64 单个 Rust 可执行文件，按需启动已安装的 Edge，使用独立持久化配置目录和本机随机 CDP 调试端口。成功后正常退出专用浏览器，保留 Gemini 登录。运行不需要 Node、npx、Rust 工具链、MCP 或常驻服务，也不依赖日常 Edge 的调试授权。

## 安装与使用

从此 fork 的 Release 下载 `ask-bridge.exe` 和 SHA-256 文件，校验后放到用户工具目录。更新时先备份旧文件再替换，不执行上游更新器。

首次执行 `login`，在新开的专用 Edge 窗口中自行登录 Gemini。程序只检查 Gemini 就绪状态，不读取 Cookie、密码或会话数据库，不复制日常 profile。登录成功并验证配置模型后正常退出，后续按需启动时复用该目录。无需在日常 Edge 的 `edge://inspect` 开启调试。

```powershell
ask-bridge config --model "4 Argon" --reasoning High
ask-bridge login
ask-bridge doctor
ask-bridge models
ask-bridge run --input draft.md --mode notice --output polished.md
```

模型名称须以本人账号菜单实际显示为准。配置不会验证模型；每次提交前从实际菜单选择并核对思考程度，不可用时报错。默认专用目录为 `%LOCALAPPDATA%\Programs\AskBridge\edge-profile`；可用 `--profile` 指定新的空目录。程序拒绝接管日常 Edge 或已有无工具标记的数据目录。同一专用目录只运行一个任务。

模式：`proofread` 校对、`notice` 通知、`academic` 学术、`opinion` 意见。默认 `--format codeblock` 要求 Gemini 返回原始 Markdown，`--format rendered` 用于普通渲染格式。可通过 `--instruction` 添加必要的少量背景。默认仅输出路径和检查摘要；`--print` 才把结果全文打印到终端。

程序在 `%LOCALAPPDATA%\Programs\AskBridge\jobs\<工作ID>` 保存原稿、发送提示、工作记录、原始回答、整理结果和检查报告。配置位于 `%USERPROFILE%\.config\ask-bridge-lite\config.json`。输出文件已存在时不覆盖。

断线或等待超时后，用报错提供的工作目录恢复，不重新发送：

```powershell
ask-bridge get --job "工作目录" --output recovered.md
```

每次新请求新建 Gemini 对话，唯一首尾标识用于排除旧回答及截断。等待时使用页面内变更观察和低频兜底检查，一次挂起的 CDP 调用；结束后退出。取回并保存结果后正常退出经启动参数验证归属的专用 Edge，保留对话链接与登录数据；失败时保留专用窗口以便登录或恢复，下一次命令可复用。不会关闭日常 Edge。

## Codex

将 `skills/ask-bridge` 和 `skills/gemini-polish` 安装到用户的 `.agents/skills`。前者提供 CLI 说明，后者处理明确点名 Gemini 的自然语言请求。普通“请润色”不触发外发。不要注册 stdio MCP。

## 格式与限制

不使用系统剪贴板。DOM 转换支持标题、强调、有序列表起始值、表格、链接和代码块；默认原始 Markdown 代码块对保真更稳妥。自动检查数字、限定词、单位和基本 Markdown 结构，语义、姓名和事实仍须人工或 Codex 核对。无法保证任意嵌套表格、数学公式或复杂网页组件完全还原。

网页结构、菜单语言和服务状态可能变化；当前模型与思考菜单按实际英文标签匹配。不静默降级，不改用 API；Google 登录和设备确认仍由用户完成。专用调试端口仅供本机使用，浏览器退出后端口关闭。High 思考可能增加服务端等待时间；本工具仅降低本机桥接开销，不承诺模型耗时或实际 token 节省比例。

## 开发、更新与卸载

`cargo fmt --all -- --check`、`cargo test --locked`、`cargo build --release --locked`；`ask-bridge self-test` 在独立离线标签页测试 DOM 提取。Windows Actions 生成可执行文件及 SHA-256；只有开发/构建需要 Rust。

更新从此 fork 下载并校验新 exe，备份替换；技能只在 CLI 用法变化时更新。卸载删除自己的 exe 和两个技能目录；确认不再需要原稿后再删除 jobs 和配置；删除专用 edge-profile 会清除其登录状态，保留它则便于重装复用。不要删除日常 Edge profile。卸载不修改其他 Codex 设置。
