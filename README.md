# PixelCode

本地桌面应用：用原创像素角色组成的"AI 团队"帮你完成编码任务。你是刘备，招募三国名将；主界面是一座像素风军营。

- 不卖模型额度，不自带模型，不读取你的 API Key。
- 需要先安装并登录 [Claude Code](https://code.claude.com) 或 Codex CLI，PixelCode 只调用你已登录的 CLI。
- 开发规格：[docs/PIXELCODE_SPEC.md](docs/PIXELCODE_SPEC.md)
- 开发状态：早期骨架，尚不可用。

## 开发

```bash
pnpm install
pnpm tauri dev   # 启动桌面应用
pnpm build       # 构建前端
pnpm typecheck   # 类型检查
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

需要 Node.js 22+、pnpm、Rust stable。

## English

PixelCode is a local desktop app that turns your already-installed coding agent CLIs (Claude Code, Codex) into a pixel-art team of Three Kingdoms generals, working in a pixel-art military camp. It does not sell tokens, ship models, or touch your API keys. See [docs/PIXELCODE_SPEC.md](docs/PIXELCODE_SPEC.md) (Chinese). Status: early skeleton.

## License

MIT
