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

## Claude Code 单角色运行（阶段 B · 任务 3）

Rust 命令 `start_role_run(campId, roleId, loadoutIndex, prompt)` 使用已保存的大营目录、
该大营生效的角色配置和选中的 Claude Code 武器槽。一次只运行一个角色；调用在运行结束时返回，
应在调用前监听当前窗口的 `role-run` 事件。事件包含 `run_id / camp_id / role_id / type`；
`type` 为 `started`、`text`（含增量 `text`）、`tool`（含 `name`）、
`completed` 或 `failed`（含 `message`）。启动前校验错误直接由命令返回。

按 Claude Code 2.1.330 的 `--help`，使用 `claude -p --output-format stream-json --verbose
--include-partial-messages`，任务内容走 stdin，人设和技能走 `--append-system-prompt`。
`--tools` 限定内置工具，角色允许/禁止规则走 `--allowedTools` / `--disallowedTools`。
本阶段没有审批界面，使用 `--permission-mode dontAsk --permission-prompts none`，
不启用交互式审批或 `--permission-prompt-tool`。命令继承用户的 CLI 登录状态；
不加载大营的项目设置或 MCP 配置。仅在信任的大营运行：工具规则不等同于文件系统沙箱，
允许编辑的角色会直接修改大营目录，工作树、任务 UI 和停止操作尚未接入。

## Codex 单角色运行（阶段 B · 任务 4）

`start_role_run` 也支持选中的 Codex 武器槽，沿用上述 `role-run` 事件与单角色运行限制。
以本机 Codex CLI 0.171.0 的 `codex exec --help` / `codex --help` 核实参数：
`codex --ask-for-approval never exec --json --sandbox read-only -`；任务经 stdin 发送，
模型非 `auto` 时传 `--model`，人设和技能通过 `--config developer_instructions=...` 发送。
JSONL 结构按官方 `codex-rs/exec/src/exec_events.rs`，配置键按官方
`codex-rs/core/config.schema.json`；`docs/exec.md` 现链接至
<https://developers.openai.com/codex/noninteractive>。

仅同时允许 Write、Edit 且未禁止任一工具时使用 `workspace-write`，否则 `read-only`。
两种模式均保留 Codex 的 shell 工具，以便通过命令读取、搜索文件及运行检查。
写沙箱关闭网络（阻止联网 git push），排除临时目录，写入限制在大营工作目录；
只读角色即使通过 shell 读取、搜索文件也无法写入。Codex 不支持逐条翻译 Claude 的
Bash deny 规则，改由沙箱约束网络与写入范围；这不等于禁止工作目录内的删除操作。
`codex exec --help` 和配置 schema 没有独立的 `.git` 只读开关；官方
`codex-rs/protocol/src/permissions.rs` 的默认工作区策略已保护 `.git` 为只读，
因此不添加未确认的参数，也不放宽该默认保护。
使用 `--ignore-user-config --ignore-rules --ephemeral --skip-git-repo-check`，避免用户/项目
配置中的 MCP、hook 或规则扩大权限，不保存会话，支持非 git 大营。CLI 登录凭证仍由 CLI 使用，
但自定义供应商配置不会加载；本适配器要求支持这些参数的版本，不做不安全降级。
仅在信任的大营运行；本阶段尚无交互审批、停止操作、真实登录运行或跨平台沙箱验证。

## 名将占位精灵（阶段 A · 任务 5）

`assets/sprites/` 下 7 名将精灵均为本仓库原创像素画（随仓库 MIT 授权），由
`python3 tools/sprites/generate.py` 从脚本内的像素字符图生成，改图请改脚本后重新生成。
每张图 4 帧 16×24：待机呼吸 ×2、行走 ×2；另有等待（黄"！"）与出错（红"！"）气泡。

`src/camp/roster.ts` 按 `roles/*.yaml` 读取名将，`avatar` 字段对应精灵文件名；
`src/camp/sprites.ts` 的 `loadGenerals()` 生成 `GeneralSprite`（脚底锚点、最近邻），
`placeGenerals()` 按角色 id 放到驻地坐标；状态到动画的映射在 `src/camp/animations.ts`（规格 §5.2）。

## 原创像素场景与界面

`themes/three-kingdoms/tiles/` 的桃园和军营均为本仓库原创像素画（MIT），
由 `python3 tools/generate_scenery.py`（需要 Pillow）可重复生成。
军营保留 320×180 逻辑分辨率与整数倍最近邻缩放；场景路径和文案由主题配置提供。
界面布局参考 [MonoCode](https://www.usemono.dev/) 的清晰分区与
[Godly](https://godly.website/) 的留白和信息层级；未复制任何第三方代码或素材。

## 队长动态编排（阶段 D · 任务 1）

队长拟定草案时会收到 `flows/*.yaml` 的四个内置参考流程（修 bug、新项目、重构、调研，规格 §7.3），
可套用、裁剪、组合或不用；非编码请求（如整理会议记录）可只招募所需名将并产出 Markdown 报告。
草案的输入产物须是其前置锦囊的首个输出产物；调度器在每个锦囊后检查产出非空，
空产出按检查失败返工（最多 3 次）。锦囊墙显示任务类型与本次招募的名将。

## English

PixelCode is a local desktop app that turns your already-installed coding agent CLIs (Claude Code, Codex) into a pixel-art team of Three Kingdoms generals, working in a pixel-art military camp. It does not sell tokens, ship models, or touch your API keys. See [docs/PIXELCODE_SPEC.md](docs/PIXELCODE_SPEC.md) (Chinese). Status: early skeleton.

## License

MIT
