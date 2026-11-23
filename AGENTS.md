# AGENTS.md — PixelCode 开发守则

给在本仓库工作的 AI 开发 Agent（如 Devin）。**开工前先读 `docs/PIXELCODE_SPEC.md`，以规格为准，规格未写的功能不做。**

## 项目一句话
本地桌面应用（Tauri 2 + Rust + React 19 + TypeScript）：用宝可梦伙伴组成的像素 AI 团队，调度用户已登录的 Claude Code / Codex CLI 完成编码任务。主界面是像素伙伴基地。**不卖模型额度，不读取、不保存、不传输用户的 API Key。**

## 常用命令
```bash
pnpm install
pnpm typecheck
pnpm build
pnpm test
pnpm tauri dev          # 打开 GUI，自动化环境不要跑
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## 提交前必须全部通过（与 CI 一致）
1. `pnpm typecheck`
2. `pnpm build`
3. `pnpm test`
4. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
5. `cargo test --manifest-path src-tauri/Cargo.toml`

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
规格已到 v0.6（`docs/PIXELCODE_SPEC.md`）。骨架与阶段 A–D 主体均已交付：SQLite（`user_version` 迁移）、CLI 探测（`adapters/{claude_code,codex}.rs` 的 `detect()`）、打开大营、军营 PixiJS 渲染、像素 UI 组件、角色/技能/主题配置、队长 plan.json + 点将/交付关卡、调度器（DAG、检查返工 ≤3、审查打回 ≤3、Git worktree 隔离 + patch 回放、最多 3 并行）、Council 对话协同、战报 PNG、中英界面、CI（Linux/Windows）。

**已知缺口**集中在规格 §17「运行时健壮性」：进程树只杀顶层进程（孙进程会残留）、重启后残留 `running` 无 `interrupted` 恢复语义、无会话续接、无 `files` 写入范围、worker 规则分散在提示词里、澄清待答只在 React state、无系统通知、缺真 spawn 伪 CLI 测试。按 §17 的 P0→P1 顺序补齐即可，不要另起炉灶。

## 接下来按规格 §17 的开发顺序（每项一个小 PR）
1. **P0 进程树终止与孤儿回收**（§17.1）：进程组启动 + 两段式终止 + `PIXELCODE_PARENT` 启动回收；配套伪 CLI 真 spawn 测试（§17.9）。
2. **P0 重启恢复语义**（§17.2）：`interrupted` 状态、启动清扫、用户显式继续、确定性 worktree 认领、集成前外部改动校验。
3. **P1 锦囊 `files` + 调度串行**（§17.4）：plan schema、点将前校验、范围重叠串行。
4. **P1 worker 规则信封 + `BLOCKED:` 上报**（§17.5）。
5. **P1 会话续接**（§17.3）：先落 session id 存储，再评估 `--resume`/`exec resume` 参数（以本机 `--help` 为准）。
6. **P1 事件与待答落库 + 系统通知**（§17.6/§17.8）。
7. **P2** 用量/限额事件（§17.7）、CLI 版本兼容表（§8.3）。
