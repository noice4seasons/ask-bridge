---
name: ask-bridge
description: 通过 Rust CLI 复用本机 Edge 的 Gemini 登录，提交指定文本并保存完整 Markdown。仅在用户明确要求 Gemini 时使用。
---

# Ask Bridge CLI

仅发送用户指定文本和必要的少量背景。保存 UTF-8 原稿后执行；不发送整段聊天或无关文件。程序使用配置中的模型和思考程度，每次提交验证菜单；不可用、额度不足或登录失效时报告，不降级或切换收费 API。

Windows 命令为 `$env:USERPROFILE\.local\bin\ask-bridge.exe`，优先用绝对路径避免旧 PATH。

```powershell
& "$env:USERPROFILE\.local\bin\ask-bridge.exe" run --input "原稿绝对路径" --mode notice --output "结果绝对路径"
```

模式 `proofread` / `notice` / `academic` / `opinion`；可用 `--instruction` 传必要背景。默认原始 Markdown 代码块返回。结果路径、警告和工作目录由 CLI 输出；读取 `result.md` 与 `audit.json`，不要把 `response.raw.md` 的首尾标识交给用户。

超时或断线只用 `get --job "工作目录"` 恢复取回，不自动重复发送；`prepared` 状态未提交，需先排除模型或登录问题。输出文件不覆盖。`doctor` 仅检查连接；`models` 查看当前网页模型；`config --model "实际标签" --reasoning "实际标签"` 保存选项。

无需 Node/npx/MCP/常驻服务。自动启动现有 Edge 的专用持久化目录，成功后退出该专用实例，保留登录；不关闭日常 Edge。首次或登录失效时执行 `login` 查看普通 Edge 手动启动步骤；让用户关闭专用调试窗口后，在普通窗口自行登录，完成后关闭窗口，再运行 `check-login` 验证模型。不通过 CDP 控制账号登录或读取凭据。专用目录为 `%LOCALAPPDATA%\Programs\AskBridge\edge-profile`，无需用户反复允许调试；失败时保留窗口，下次命令复用。原稿和记录位于 `%LOCALAPPDATA%\Programs\AskBridge\jobs`，配置位于 `%USERPROFILE%\.config\ask-bridge-lite\config.json`。
