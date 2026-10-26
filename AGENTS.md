# Gemini Edge Lite 开发规则

此 fork 仅支持 Gemini 网页和现有 Edge 登录，保留上游 MIT 署名。

- 使用 Rust 单执行文件，直接 CDP；运行不依赖 Node、npx、MCP 或常驻服务。
- 仅在用户明确要求 Gemini 时发送指定文本。不要读取登录凭据、复制 profile、操作其他对话或切换收费方式。
- 提交前保存原稿和工作记录；超时或断线只从原工作取回，不自动重发。
- 仅连接本机调试端点，操作本次创建的 Gemini 页面，不关闭用户的 Edge。
- 保留 UTF-8、Markdown、数字、单位和结论强度；不完整回答明确报错。
- 运行 cargo fmt --all -- --check、cargo test --locked、cargo build --release --locked；浏览器相关修改以 self-test 和虚构中文样稿验证。
- 说明和提交信息用简体中文。提交说明实际变更和测试；不上传原稿、账号数据或个人路径配置。
