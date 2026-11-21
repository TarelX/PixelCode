# AGENTS.md — PixelCode 开发守则

给在本仓库工作的 AI 开发 Agent（如 Devin）。**开工前先读 `docs/PIXELCODE_SPEC.md`，以规格为准，规格未写的功能不做。**

## 项目一句话
本地桌面应用（Tauri 2 + Rust + React 19 + TypeScript）：用三国名将组成的像素 AI 团队，调度用户已登录的 Claude Code / Codex CLI 完成编码任务。主界面是像素军营。**不卖模型额度，不读取、不保存、不传输用户的 API Key。**

## 常用命令
```bash
pnpm install
pnpm typecheck
pnpm build
pnpm tauri dev          # 打开 GUI，自动化环境不要跑
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## 提交前必须全部通过（与 CI 一致）
1. `pnpm typecheck`
2. `pnpm build`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo test --manifest-path src-tauri/Cargo.toml`

任何一项失败不得提交、不得开 PR。

## 工作流
- **一个阶段、一个任务一个 PR**，从 `main` 切分支 `feat/<阶段>-<简述>`，如 `feat/a-camp-tilemap`。PR 小而可审（建议 < 500 行 diff）。
- 禁止直接推 `main`，禁止 force push，禁止改写已推送的历史。
- 提交信息中文或英文均可，代码标识符一律英文；一次提交只做一件事。
- PR 描述写：做了什么、对应规格哪一节、如何验证、已知缺口。
- 不确定规格含义、或规格与现实冲突（如 CLI 参数变了）时：**在 PR 里写明并采用保守方案，不要猜测不存在的 flag**；以本机 `--help` 与官方文档为准。
- 每个阶段结束必须是可运行状态，不要把半成品堆在一起。
- 阶段顺序：A（军营空壳）→ B（角色系统 + 单角色执行）→ C（队长 + 锦囊墙 + 编排）→ D（动态流程 + 完善）。**不要跨阶段提前做。** 阶段验收标准见规格 §12。

## 架构硬约束（来自规格）
- 前端不得直接 spawn 命令；所有 git 与 Agent 进程操作走 Rust 侧，参数逐个传递，**禁止 `shell: true` / 字符串拼接命令**。
- 每个 CLI 适配器单独一个文件（`src-tauri/src/adapters/`），禁止巨型 switch。适配器要有 fixture 测试（原始输出样本 → 期望事件），放 `tests/adapters/fixtures/`。
- 调度器不变量（规格 §7.4）：点将 → 执行 → 检查 → 审查 → 交付；检查失败返工 ≤3 次，审查打回 ≤3 轮；依赖必须是 DAG；git 提交/合并必须用户确认。**由调度器程序运行检查命令，不信 Agent 自述"已完成"。**
- worktree 与产物放应用数据目录，路径必须规范化，禁止越出允许根目录。
- 像素渲染必须整数倍缩放 + 最近邻（`image-rendering: pixelated` / PixiJS `scaleMode: 'nearest'`），禁止模糊缩放、渐变、圆角抗锯齿。
- 角色名、叙事文案、军营素材走 `themes/` 与 `roles/` 配置，**不要硬编码在代码里**。
- SQLite 表结构见规格 §10；消息量大，不要把聊天塞进单个 JSON。
- 不引入任何需要登录本应用云服务的依赖，不上传任何用户代码/对话。

## 代码风格
- 紧凑、惯用；不加多余注释，不写冗长错误处理。Rust 与 TS 都遵循现有风格，先看相邻文件。
- 错误要向上回传并在 UI 展示可读原因（如"无法启动 Claude Code：未找到 claude 命令"），不要只写"出错了"。
- 新增依赖前先确认必要性；优先选发布满 7 天以上的版本，不用浮动范围（`*`、`latest`）。**不要为了让 CI 通过而修改安全策略或锁版本规则，遇到阻塞写进 PR 求助。**
- 不提交密钥、`.env`、本机路径。

## 素材与许可
- 像素素材必须原创或可商用开源许可；第三方素材须在 README 列出作者与协议，不得使用带"禁止再分发"的素材。
- 角色一律原创像素形象，不得使用动漫/游戏 IP 形象。
- 应用内不得宣称与任何第三方游戏或品牌官方相关。

## 当前状态
骨架已搭好：Tauri 2 + React 19 + zustand + pixi.js，目录按规格 §11，7 个预设角色 YAML、`three-kingdoms` 主题、6 个技能占位、CI。**所有业务模块仍是空占位**，从阶段 A 开始实现。

## 阶段 A 任务清单（按顺序，每项一个 PR）
1. **SQLite 初始化**：`src-tauri/src/db.rs`，在应用数据目录创建库并建 `camps / roles / tasks / plans / artifacts / messages / app_settings` 表（规格 §10），带迁移版本号与单元测试。
2. **CLI 探测**：`adapters/claude_code.rs`、`adapters/codex.rs` 的 `detect()`，返回 `not_found / found / found_needs_login`，Tauri command 暴露给前端；先在本机用 `--help` 确认 `claude auth status`、`codex` 的真实可用命令，再写。
3. **打开大营**：标题屏"招募名将"→ 系统文件夹选择 → 校验是目录 → 写入 `camps` 并进入 `CampView`；记住最近 5 个路径。
4. **军营渲染骨架**：PixiJS 画布挂进 `CampView`，320×180 逻辑分辨率整倍放大、最近邻；先用纯色图块占位画出中军帐 / 点将台 / 演武场 / 城门 / 各驻地的位置（布局见规格 §5.1）。
5. **名将占位精灵**：按 `roles/*.yaml` 加载 7 名将，用色块占位角色站在各自驻地，播放简单待机呼吸动画；状态到动画的映射见规格 §5.2。
6. **像素 UI 基础组件**：按钮、对话框（9-slice 风格边框）、顶栏；样式用 `src/styles/` 的调色盘变量。

阶段 A 验收：打开文件夹能看到军营俯视图，名将站在各自驻地（规格 §12）。
