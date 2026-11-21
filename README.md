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

## CLI 探测（阶段 A · 任务 2）

按规格 §8.3 / §9.3，启动及进入设置时通过 Rust 的 `detect_clis` Tauri command
扫描当前进程的 PATH；前端 `useCliDetection` 只调用 command，不启动进程。
返回每个 CLI 的 `id / status / version / error`，状态为
`not_found / found / found_needs_login`。未安装 CLI 不影响打开大营。

本机通过 `--help` 核实的命令（Claude Code 2.1.330、Codex CLI 0.171.0）：
- `claude --version`、`claude auth status --json`（默认 JSON 的 `loggedIn` 布尔字段）。
- `codex --version`、`codex login status`（登录状态文本可能在 stderr）。

规格以 `codex --version` 为无害探测示例，但版本不证明登录，因此用真实存在的
`login status` 检查登录。不会运行推理、发起登录、读取凭证文件或回传原始认证输出。
每条命令最多等待 5 秒。旧版本不支持状态命令、输出未知、权限错误或超时时，
保守返回 `found_needs_login` 并附可读错误；界面显示“无法确认状态”，不误报已登录。
PATH 指应用启动时继承的环境（GUI 启动时可能与终端不同）。

适配器 fixture 覆盖已登录、未登录、空输出与错误输出。
fixture 中已登录输出为合成样本，未登录格式已在本机验证。
新建任务目前仍为空占位；规格要求的无可用 CLI 时拦截任务应在实现新建任务时接入，
不得仅凭 `version` 或有 `error` 的结果放行。

## English

PixelCode is a local desktop app that turns your already-installed coding agent CLIs (Claude Code, Codex) into a pixel-art team of Three Kingdoms generals, working in a pixel-art military camp. It does not sell tokens, ship models, or touch your API keys. See [docs/PIXELCODE_SPEC.md](docs/PIXELCODE_SPEC.md) (Chinese). Status: early skeleton.

## License

MIT
