# PixelCode 开发规格

版本：0.6  
日期：2026-11-23（Asia/Singapore，UTC+8）  
状态：可直接按本文实现 v1  
本文是给实现者的开发规格，不是营销文案。实现时以本文为准；本文未写的功能一律不做。

v0.6 变更：新增 §17 运行时健壮性要求（进程树终止、重启恢复、中断/失败区分、会话续接、写入范围调度、worker 规则信封、集成前校验、事件持久化、系统通知、测试策略）；§9.3 适配器接口改为现行 headless JSONL 形态；§8.3 探测区分"已安装 / 已登录 / 版本兼容"；§10 任务状态增加 `interrupted`；§7.2 锦囊增加 `files` 写入范围字段。

---

## 1. 产品一句话

PixelCode 是一个本地桌面应用：用原创像素角色组成的"AI 团队"帮用户完成编码任务。产品叙事为"你是刘备，招募名将"——每个 AI 角色是一位三国名将，任务卡在 UI 中称"锦囊"，用户确认计划的关卡称"点将"。主界面是一座像素风军营。用户只需安装并登录 Claude Code 或 Codex，本应用负责组队、分派、执行、检查、审查和交付。应用不卖模型额度，不自带模型。

英文名：PixelCode  
仓库/包名：`pixelcode`

---

## 2. 定位：做什么、不做什么

### 2.1 做什么

- 把用户已经安装并登录的编码 Agent CLI（第一版：Claude Code、Codex）组织成有分工的"角色团队"。
- 用像素风军营界面展示角色状态、锦囊流转、产物交接、审查结果。
- 提供两种模式：
  - **对话协同**（v1.5）：多个角色在同一对话里讨论，帮用户把需求想清楚。
  - **结构化编排**（v1）：队长拆任务，角色按依赖接力，调度器执行检查和审查。
- 支持任意编码任务：队长根据用户输入动态编排流程（修 bug、开发新系统、重构、调研等都能处理）。

### 2.2 明确不做（v1 和非目标）

- 不销售、转发、代理任何模型 token，不要求用户把 API Key 交给本应用。
- 不自己实现一套编码 Agent，不替代 Claude Code / Codex 的推理能力。
- 不做云同步、账号系统、多人协作、远程 Agent。
- 不做插件市场、自定义主题编辑器、手机端。
- 不做知识库/RAG、PPT 生成、图片生成（v2+ 再考虑）。
- 不做"对话协同"模式（v1.5 再做）。
- 不要做成 monocode / Codex / Crow5 的像素换皮：核心差异是"角色团队 + 结构化编排 + 军营可视化"。

### 2.3 竞品与差异

| 竞品 | 它做什么 | 我们的差异 |
| --- | --- | --- |
| monocode | 多 Agent 会话管理，标签页 + worktree，10+ CLI | 我们有角色分工、队长动态编排、军营可视化；它没有 |
| Crow5 | 闭源商业 AI 工作站，多 Agent 对话协同，覆盖面广 | 我们开源、本地、用用户自己的订阅；协作是结构化编排而非对话 |
| Pixel Agents | 像素风 Agent 状态监控（办公室场景） | 我们有任务编排、角色技能、审查流程；场景是军营不是办公室 |
| ccbridge / RoleMux / oh-my-agent | 命令行多角色流水线 | 我们有图形界面、军营可视化、人工关卡 |

---

## 3. 核心概念

| 概念 | 含义 |
| --- | --- |
| 大营（Camp） | 一个工作目录（本地文件夹或 git 仓库）。一个窗口同时只打开一个大营。 |
| 角色（Role） | 一个 Agent 人设，包含：人设提示词、技能列表、武器槽（绑定哪个 CLI）。 |
| 武器槽（Loadout） | 角色可用的 CLI + 模型组合，如"Claude Code + sonnet"。 |
| 技能（Skill） | 角色挂载的能力包，如设计规范、前端组件库用法、测试规范。 |
| 锦囊（Task Card） | 队长拆解出的一个具体任务，写明角色、依赖、输入、输出、验收标准、检查命令、`files` 写入范围（§17.4）。 |
| 队长（Captain） | 一个特殊角色（诸葛亮），负责把用户需求拆成锦囊，或在手动模式下协助分配。 |
| 调度器（Scheduler） | 本应用的核心程序，负责按依赖执行任务、运行检查、触发审查、管理 git。 |
| 产物（Artifact） | 角色之间交接的文件，如 design.md、api-contract.md、diff、review.md。 |
| 点将（Gate） | 需要用户确认的节点：方案确认 = 【点将】，交付确认 = 【交付】。 |
| 战报（Battle Report） | 导出的今日进度图，军营状态的像素快照。 |

---

## 4. 角色系统

产品叙事：用户扮演刘备，AI 角色是三国名将。界面默认简体中文，角色同时提供中文名与英文名（英文界面时显示英文名）。任务卡在 UI 中称"锦囊"；用户确认计划的关卡称"点将"。

### 4.1 角色定义

每个角色是一个配置文件（JSON/YAML），包含：

```yaml
id: web-designer               # 内部 ID 用英文，稳定不变
name: 貂蝉                     # 中文主名
name_en: Diao Chan             # 英文名（英文界面显示）
title: 网页设计师               # 职业称谓
avatar: diao-chan.png          # 16x16 或 16x24 像素精灵
personality: 你是貂蝉，网页设计师，负责……（人设提示词）
skills:                          # 挂载的技能
  - pixel-design-basics
  - component-library-usage
loadouts:                        # 武器槽：可用 CLI 列表
  - cli: claude-code
    model: sonnet
  - cli: codex
    model: auto
permissions:                     # 工具权限
  allow: ["Read", "Write", "Edit"]
  deny: ["Bash(git push *)", "Bash(rm -rf *)"]
task_types: ["frontend", "ui", "design"]
```

### 4.2 v1 预设角色

| 名将 | 职责 | 中文名 / 英文名 | 典型产物 |
| --- | --- | --- | --- |
| 诸葛亮（孔明） | 队长：拆任务、分派名将、汇总结果 | 诸葛亮 / Zhuge Liang | plan.json（锦囊）、summary.md |
| 庞统（凤雏） | 架构师：系统设计、接口约定 | 庞统 / Pang Tong | architecture.md、api-contract.md |
| 貂蝉 | 网页设计师：UI/UX 设计、设计稿描述 | 貂蝉 / Diao Chan | design.md、wireframe.md |
| 吕布 | 前端工程师：实现前端代码 | 吕布 / Lü Bu | 代码、组件、测试 |
| 关羽 | 后端工程师：实现后端代码、API | 关羽 / Guan Yu | 代码、API、测试 |
| 徐庶 | 调试员：复现 bug、定位根因、修复 | 徐庶 / Xu Shu | repro.md、fix.md |
| 张昭 | 审查员：审查代码质量、安全性、规范 | 张昭 / Zhang Zhao | review.md（通过/打回+问题清单） |

扩展名将（v2+ 预留）：赵云（测试工程师）、张飞（实现工程师）、姜维（重构工程师）、黄月英（工具工程师）。
备选替换：架构师可换司马懿；调试员可换华佗；审查员可换黄忠或郭嘉。

用户可以：
- 修改角色的人设、技能、武器槽。
- 新建自定义角色。
- 为同一角色配置多个武器槽，队长或用户选择用哪个。

### 4.3 技能（Skill）

技能是角色的"装备"，本质是提示词片段 + 可选的工具配置。v1 内置：

- `pixel-design-basics`：像素风设计规范（配色、间距、组件）
- `frontend-component-usage`：前端组件库用法
- `api-design-conventions`：API 设计规范
- `test-writing`：测试编写规范
- `code-review-checklist`：代码审查清单
- `bug-reproduction`：bug 复现方法

用户可以自定义技能（Markdown 文件 + 可选 YAML 配置）。

### 4.4 主题皮肤（Theme Pack）

角色名、叙事文案、军营素材一律走主题包配置，**代码零硬编码**：

```
themes/
  three-kingdoms/            # v1 唯一内置主题
    names.yaml               # 诸葛亮 / Zhuge Liang ……
    lore.md                  # "你是刘备"叙事文案
    tiles/ sprites/          # 军营素材
  generic-heroes/            # 预留，v2+ 再做
    names.yaml               # Captain / Architect ……
```

- v1 只实现 `three-kingdoms`。
- 英文名用标准罗马化写法（Zhuge Liang、Diao Chan），配英文称谓（"Zhuge Liang · The Strategist"），不认识三国的用户也能看懂职责。
- 将来如需面向国际用户，新增 `generic-heroes` 主题包即可，纯 YAML + 素材，不改代码。

---

## 5. 界面结构：军营

主界面是一张俯视的像素军营地图。逻辑分辨率 320×180 或 384×216，16×16 图块，整倍最近邻放大，禁止模糊缩放。

### 5.1 军营布局

| 区域 | 功能 | 对应 |
| --- | --- | --- |
| 中军帐 | 队长驻地，显示当前计划与状态 | 诸葛亮 |
| 点将台 | 【点将】关卡：锦囊与名将分配的确认界面 | 用户确认 |
| 演武场 | 名将"操练"= 执行锦囊的主区域 | 执行中的角色 |
| 城门/关隘 | 【交付】关卡：成果出关的地方 | 交付确认 |
| 兵部/审核堂 | 审查员驻地 | 张昭 |
| 水榭/花园 | 设计师驻地 | 貂蝉 |
| 各人驻地 | 每个名将有固定岗位，空闲时站在驻地 | 全体名将 |

### 5.2 名将状态与动画

- `running`：在营地内行走/操练（工作动画）。
- `waiting_user`：停在原地循环挥手，头顶"！"气泡。
- `done`：走向城门方向待机（凯旋预备）。
- `error`：停在驻地，头顶红色感叹号气泡。
- `idle`：驻地待机，轻微呼吸动画。
- `cancelled`：退到营地边缘，不播放工作动画。

产物交接：信使在名将之间传递锦囊卷轴。锦囊从队长处发出，依次流转到各执行名将，再到张昭审核堂，最后经城门交付。

### 5.3 其他界面

- **标题屏**：桃园场景，桃花飘落，主按钮"招募名将"（= 打开大营/选择目录）。
- **锦囊详情**：点开某个锦囊，显示负责名将、依赖、输入/输出产物、diff、审查意见。
- **名册侧栏**：可收起的辅助面板，列出全部名将与状态；主导航始终是军营地图。
- **战报导出**：把当天军营状态导出为一张 PNG 战报（见 6.3 进度图要求）。

---

## 6. 协作模式

### 6.1 结构化编排（v1 核心）

流程：

```
用户输入需求
   ↓
队长分析（只读）→ 输出问题列表 或 直接出计划
   ↓  有问题 → 弹窗问用户 → 用户回答或"你来定"
队长输出 plan.json（锦囊列表 + 本次招募的名将）
   ↓
【点将】用户审阅锦囊与名将分配，可修改，可批准
   ↓
调度器按依赖执行任务（Git 大营中独立任务在 worktree 内并行，最多 3 个；非 Git 大营顺序执行）。任务卡可声明 `files` 写入范围（见 §17.4），范围重叠的任务即使无依赖也串行执行。
   ↓  每个任务：
   ├─ 调度器启动角色进程（claude -p / codex exec）
   ├─ 角色读取输入产物，执行任务，写输出产物
   ├─ 调度器运行检查命令（如 npm test）
   │   ├─ 通过 → 进入审查
   │   └─ 失败 → 把错误交回角色返工（最多 3 次）
   ├─ 审查员审查（全新会话，只读）
   │   ├─ 通过 → 盖章，git commit
   │   └─ 打回 → 返工（最多 3 轮）
   └─ 超过上限 → 标记 blocked，交给用户决定
   ↓
【交付】用户确认交付
```

### 6.2 对话协同（v1.5，预留接口）

- 用户说需求，多个角色在同一对话里讨论。
- 每个角色有独立的系统提示词和模型。
- 系统汇总讨论，输出方案。
- 方案确认后，可进入结构化编排模式。

v1 不实现，但数据模型和角色系统要预留扩展点。

### 6.3 手动 vs 自动

- **自动模式**：队长根据任务类型和角色名册分配角色。
- **手动模式**：用户在界面上把角色拖到锦囊上，或下拉选择。
- 两者用同一套数据结构，只是"谁填表"不同。

---

## 7. 流程生成：由用户输入决定

### 7.1 核心原则

**流程不固定死。** 队长（诸葛亮）根据用户输入动态决定：这是什么任务、派哪些名将、怎么拆锦囊。用户说"修个 bug"和说"做个管理系统"，出来的流程应该完全不同；用户说"把这份 Excel 整理成报表"，也应该能生成对应流程。

### 7.2 队长的工作

队长分析用户输入后，输出 plan.json，包含：

- `scenario`：任务类型标签（bug-fix / new-project / refactor / research / custom……仅用于展示和统计，不驱动流程）
- `recruited_roles`：本次任务启用哪些名将（不要求全员出动；也可以临时招募名册外角色）
- `tasks`：锦囊列表，每个锦囊写明负责名将、依赖、输入产物、输出产物、验收标准、检查命令、`files` 写入范围（见 §17.4）

队长可以向用户提问（澄清需求），也可以在用户说"你来定"时自行决定。

### 7.3 内置模板 = 参考答案，不是固定流程

为降低队长出错概率，内置少量流程模板作为**参考**：

| 模板 | 参考流程 | 适用 |
| --- | --- | --- |
| 修 bug | 复现 → 修复 → 检查 → 审查 → 交付 | 有缺陷要修 |
| 新项目 | 澄清 → 架构 → 设计 → 前端 → 后端 → 测试 → 交付 | 从零做系统 |
| 重构 | 补测试 → 重构 → 测试保持全绿 → 审查 | 改结构不改行为 |
| 调研 | 收集 → 分析 → 报告 | 不改代码，只产出结论 |

队长可以**套用、裁剪、组合或完全无视**这些模板，按输入复杂度自行编排。模板只保证"最小可行结构"，不是流程上限。

### 7.4 调度器保证的不变量（无论流程怎么变）

- 计划 → 【点将】用户确认 → 执行 → 检查 → 审查 → 【交付】用户确认
- 检查命令失败自动返工（上限 3 次）；审查打回自动返工（上限 3 轮）
- 所有 git 操作由程序执行；合并/提交必须用户确认
- 任务依赖必须是 DAG（有向无环图），调度器负责拓扑排序

### 7.5 流程示例（队长可能产出的结果，非固定）

**示例 A：用户说"登录接口有 bug，刷新后掉登录"**

队长判断为 bug-fix，招募徐庶（调试）+ 关羽（后端）+ 张昭（审查）：

```
用户拖入仓库/文件
   ↓
调度器建立工作目录，git init，提交基线
   ↓
队长分析 → 锦囊1：徐庶复现 bug（写失败测试/脚本）
        → 锦囊2：关羽修复（依赖锦囊1的复现脚本）
        → 锦囊3：张昭审查 diff
   ↓
【点将】用户确认
   ↓
逐锦囊执行：角色干活 → 调度器跑检查 → 张昭审查
   ↓
【交付】用户确认 → 交付补丁/文件
```

**示例 B：用户说"做一个图书管理系统"**

队长判断为 new-project，招募全员：

```
用户输入需求
   ↓
队长澄清（技术栈、功能范围、是否要登录……）→ 用户回答或"你来定"
   ↓
队长输出锦囊：庞统架构 → 貂蝉设计 → 吕布前端 → 关羽后端 → 张昭审查
   ↓
【点将】用户确认（可调整锦囊）
   ↓
按依赖接力执行，每步检查 + 审查
   ↓
【交付】用户确认 → 交付可运行项目
```

**示例 C：用户说"把这份会议记录整理成行动项清单"**

队长判断为非编码任务，可能只招募诸葛亮（拆解）+ 张昭（审查），产物是 markdown 报告而非代码。调度器的检查命令可以是"文件存在且非空"这类轻量校验。

### 7.6 手动模式的角色

用户可以跳过队长，自己拖角色到锦囊上、自己写依赖关系。此时队长只做"建议"，调度器仍按同样的不变量执行。

---

## 8. 模型来源（不碰用户密钥）

### 8.1 原则

- **只调用用户已安装、已登录的 CLI**。
- **不读取、不保存、不传输用户的 API Key 或订阅凭证**。
- 用户在 CLI 里配置供应商（直接配或用 cc-switch 等工具），我们不管。

### 8.2 v1 支持的 CLI

| CLI | 启动方式 | 结构化输出 | 权限审批 | 状态 |
| --- | --- | --- | --- | --- |
| Claude Code | `claude -p` | `--output-format stream-json`、`--json-schema` | `--permission-prompt-tool` | 必须实现 |
| Codex | `codex exec` | JSON 输出 | 需实测 | 必须实现 |

### 8.3 探测

启动时和进入设置时扫描 `PATH`。检测分三件事，UI 分别展示，不合并成一个布尔值：

- **已安装**：`PATH` 中找到可执行文件（或用户配置的显式路径）。
- **已登录**：用无害命令确认会话有效，如 `claude auth status --json` 的 `loggedIn`、`codex login status` 的 `Logged in using ...`。
- **版本兼容**：当前 CLI 版本是否在该适配器已验证的参数集内。适配器维护一份已验证版本/能力表；不在表内时标记"未验证"并给出保守提示，不静默按旧参数启动。

显示"已找到 / 未找到 / 已找到但未登录 / 版本未验证"。未找到任何 CLI 时，打开大营后界面可用，但新建任务会被拦截，并给出安装文档链接。

新增第三个 CLI 之前，必须先把各适配器的版本/能力、会话生命周期、目录探测、用量上报、审批处理抽成统一的 adapter trait（见 §9.3 与 §17），不允许再长出第二种传输形态。

### 8.4 未来扩展

- Gemini CLI、Cursor、DeepSeek 等，通过通用适配器慢慢加。
- 兼容 API（OpenAI 格式）的 CLI，通过用户自己配置的供应商接入。

---

## 9. 技术架构

### 9.1 推荐技术栈

- 桌面：Tauri 2 + Rust
- 前端：React 18+ + TypeScript + Vite
- 状态：Zustand
- 军营渲染：HTML Canvas 或 PixiJS（推荐 PixiJS，最近邻缩放）
- 本地数据：SQLite（Tauri 插件或 Rust）
- 进程管理：Rust 侧用 `std::process::Command` 启动 CLI 子进程，stdin 传任务、stdout/stderr 逐行解析 JSONL/JSON 事件，经 Tauri event 把流式更新推到前端。**不用 PTY**：本应用只做 headless 单轮执行，不需要伪终端；需要会话续接时按 §17.3 走 CLI 自己的 resume 参数。
- Git：调用系统 `git`，不自己实现

### 9.2 进程边界

```
┌─────────────────────────────────────────┐
│  Frontend (React)                       │
│  军营俯视图 / 锦囊墙 / 名将卡 / 产物查看   │
└──────────────▲──────────────────────────┘
               │ Tauri commands + events
┌──────────────▼──────────────────────────┐
│  Rust core                              │
│  - CampStore (SQLite)                   │
│  - RoleRegistry (角色配置)               │
│  - ThemeRegistry (主题皮肤)              │
│  - SkillRegistry (技能配置)              │
│  - Scheduler (任务调度、检查、审查)       │
│  - RunSlot/supervisor (spawn/kill 进程树) │
│  - AdapterRegistry (CLI 适配器)          │
│  - GitService (worktree/branch/diff)    │
└──────────────▲──────────────────────────┘
               │ child process, cwd=worktree, stdin/stdout JSONL 事件流
┌──────────────▼──────────────────────────┐
│  用户已安装的 CLI                       │
│  claude / codex                         │
└─────────────────────────────────────────┘
```

### 9.3 适配器接口

适配器是纯函数式的协议模块：把 CLI 的 argv/stdin 协议与结构化事件解析封装在一起，调度器只面对统一的事件类型。现行形态（每个适配器一个文件，禁止巨型 switch）：

```rust
// src-tauri/src/adapters/<cli>.rs
pub fn detect() -> DetectResult;                 // 已安装 / 已登录 / 版本兼容三态
pub(crate) fn start(
    options: StartOptions<'_>,                   // cwd, model, system_prompt, prompt, permissions
    control: &RunSlot,                           // 进程注册 + 停止/终止入口
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<String, String>;                     // 返回最终文本
```

- 启动参数逐个传递，任务内容走 stdin，**不进 argv**；禁止 `shell: true` / 字符串拼接。
- 事件流统一为 `RunUpdate`：`Started / Text / Tool / Waiting / Completed / Stopped / Failed`；后续按 §17.7 扩展 `usage_limited` / `turn_metrics` 等变体。
- 每个适配器必须有 fixture 测试：`tests/adapters/fixtures/` 放登录探测 JSON 与启动 JSONL 样本（成功、错误、畸形行），断言解析出的事件序列与最终文本。
- 不认识的事件类型必须安全跳过（向前兼容 CLI 新增事件），不允许 panic。
- 适配器只做协议翻译：权限由角色 `permissions` 映射为 CLI 参数（Claude `--allowedTools/--disallowedTools`、Codex `--sandbox`），不在适配器里发明第二套权限模型。

### 9.4 安全

- 所有 git 与 Agent 操作走 Rust 侧，参数转义，禁止 `shell: true` 拼接。
- worktree 放在应用数据目录，不放进仓库内部。
- 不上传任何数据到本应用服务器（MVP 没有服务器）。
- 日志默认不写文件内容全文。

---

## 10. 数据模型

SQLite 建议表（字段名保持英文）：

**camps**（原 workshops，语义为"大营"）
- `id` TEXT PK
- `name` TEXT
- `root_path` TEXT UNIQUE NOT NULL
- `created_at` INTEGER
- `last_opened_at` INTEGER

**roles**（角色实例，用户可修改）
- `id` TEXT PK
- `camp_id` TEXT NULL（NULL 表示全局角色）
- `role_type` TEXT NOT NULL（captain, architect, designer, frontend, backend, debugger, reviewer）
- `name` TEXT NOT NULL
- `name_en` TEXT NULL
- `title` TEXT NULL
- `avatar` TEXT
- `personality` TEXT NOT NULL
- `skills_json` TEXT NOT NULL
- `loadouts_json` TEXT NOT NULL
- `permissions_json` TEXT NOT NULL
- `created_at` INTEGER
- `updated_at` INTEGER

**tasks**（锦囊）
- `id` TEXT PK
- `camp_id` TEXT NOT NULL
- `plan_id` TEXT NOT NULL
- `title` TEXT NOT NULL
- `description` TEXT NOT NULL
- `role_id` TEXT NOT NULL
- `depends_on` TEXT NULL（JSON 数组，依赖的任务 id）
- `input_artifacts` TEXT NULL（JSON 数组）
- `output_artifacts` TEXT NULL（JSON 数组）
- `acceptance_criteria` TEXT NOT NULL
- `check_command` TEXT NULL
- `status` TEXT NOT NULL（pending, running, checking, reviewing, done, blocked, failed, interrupted）
- `attempts` INTEGER DEFAULT 0
- `review_rounds` INTEGER DEFAULT 0
- `created_at` INTEGER
- `updated_at` INTEGER

**plans**（队长输出的计划）
- `id` TEXT PK
- `camp_id` TEXT NOT NULL
- `scenario` TEXT NOT NULL（bug-fix, new-project, refactor, research, custom……）
- `user_request` TEXT NOT NULL
- `plan_json` TEXT NOT NULL
- `status` TEXT NOT NULL（draft, approved, executing, completed, aborted）
- `created_at` INTEGER

**artifacts**（产物）
- `id` TEXT PK
- `task_id` TEXT NOT NULL
- `type` TEXT NOT NULL（design, contract, code, diff, review, ...）
- `file_path` TEXT NOT NULL
- `content_preview` TEXT NULL
- `created_at` INTEGER

**messages**（角色运行日志）
- `id` TEXT PK
- `task_id` TEXT NOT NULL
- `role` TEXT NOT NULL（user, assistant, tool, system, raw）
- `type` TEXT NOT NULL
- `content` TEXT NOT NULL
- `payload_json` TEXT NULL
- `created_at` INTEGER

**app_settings**
- 默认 CLI、缩放、通知开关、语言、窗口尺寸、当前主题

---

## 11. 建议目录结构

```
pixelcode/
  README.md
  LICENSE                    # MIT
  package.json
  src/
    app/
      App.tsx
      store.ts
    ui/
      TitleScreen.tsx
      CampView.tsx           # 军营主界面
      TaskBoard.tsx          # 锦囊墙（任务板）
      RoleCard.tsx           # 名将卡
      TaskDetail.tsx         # 锦囊详情（产物、diff、审查）
      SettingsView.tsx
      PixelDialog.tsx
      components/
    camp/
      tilemap.ts
      sprites.ts
      animations.ts
    orchestrator/
      scheduler.ts            # 前端调度逻辑（实际执行在 Rust）
      plan-parser.ts
      artifact-manager.ts
    i18n/
      zh.ts
      en.ts
    styles/
      palette.css
      pixels.css
  src-tauri/
    src/
      main.rs
      lib.rs
      db.rs
      roles.rs
      themes.rs
      scheduler.rs
      git.rs
      supervisor.rs
      adapters/
        mod.rs
        claude_code.rs
        codex.rs
    tauri.conf.json
  assets/
    sprites/                  # 原创像素名将
    tiles/
    ui/
    fonts/                    # 像素字体，需确认许可
  themes/
    three-kingdoms/           # v1 内置主题
      names.yaml
      lore.md
      tiles/
      sprites/
    generic-heroes/           # 预留
  roles/                      # 预设角色配置（文件名用拼音，内部 id 用英文）
    kongming.yaml              # 诸葛亮 · 队长
    pangtong.yaml              # 庞统 · 架构师
    diaochan.yaml              # 貂蝉 · 网页设计师
    lvbu.yaml                  # 吕布 · 前端
    guanyu.yaml                # 关羽 · 后端
    xushu.yaml                 # 徐庶 · 调试员
    zhangzhao.yaml             # 张昭 · 审查员
  skills/                     # 预设技能
    pixel-design-basics/
    frontend-component-usage/
    api-design-conventions/
    test-writing/
    code-review-checklist/
    bug-reproduction/
  tests/
    adapters/
      fixtures/
    scheduler/
  docs/
    PIXELCODE_SPEC.md          # 本规格（仓库内副本；本地文件名仍为 PIXEL_CODE_WORKSHOP_SPEC.md）
```

---

## 12. 实现阶段

### 阶段 A：军营空壳（先可打开、可看）

- Tauri 应用启动、标题屏（桃园）、打开文件夹/仓库。
- 军营俯视图渲染：营帐、中军帐、演武场、城门、桃树、角色占位动画。
- SQLite 初始化。
- 像素 UI 基础组件：按钮、对话框、顶栏。
- CLI 探测（Claude Code、Codex）。

验收：打开文件夹能看到军营俯视图，名将站在各自驻地。

### 阶段 B：角色系统 + 单角色执行

- 角色配置加载（预设 + 自定义）。
- 单角色运行：启动 `claude -p` 或 `codex exec`，流式输出显示在角色头顶的对话气泡。
- 产物文件读写。
- 审查员角色（只读）。

验收：能选一个名将，给它一个任务，看它干活，产物文件出现在指定位置。

### 阶段 C：队长 + 锦囊墙 + 编排

- 队长生成 plan.json。
- 锦囊墙 UI：锦囊卡、依赖连线、状态。
- 调度器：按依赖顺序执行任务，运行检查命令，触发审查。
- 【点将】（方案确认）、【交付】（交付确认）两个关卡。
- 修 bug 类任务完整跑通（队长应自动生成复现→修复→审查流程）。

验收：用户输入 bug 描述，队长出锦囊，用户点将确认，调度器自动执行，检查通过，审查通过，交付补丁。

### 阶段 D：动态流程 + 完善

- 队长动态编排：非 bug/新项目类任务也能生成合理流程。
- 并行执行（独立任务用 worktree）。
- 对话协同模式（v1.5）。
- 战报导出 PNG。
- 中英界面。
- README、测试、打包。

验收：能从零开发一个小型 Web 应用，全程军营可视化；也能处理非编码任务（如整理文档）。

---

## 13. 验收标准（v1 完成的定义）

同时满足以下条件才算 v1 完成：

1. 在 Windows、macOS 或 Linux 至少一个平台上，能打开本地文件夹/仓库。
2. 能探测到 Claude Code 或 Codex，且已登录。
3. 能创建名将团队，队长能生成锦囊计划。
4. 能按计划执行任务，名将产出文件，调度器能运行检查命令。
5. 张昭能审查并给出通过/打回。
6. 修 bug 类任务能完整跑通：复现 → 修复 → 检查 → 审查 → 交付。
7. 新项目类任务能跑通：架构 → 设计 → 前端 → 后端 → 测试 → 交付。
8. 军营俯视图能显示名将状态、锦囊流转、产物交接。
9. 未安装任何 CLI 时有明确引导，应用不崩溃。
10. README 写清：本应用不提供模型额度；如何安装；已知限制。

---

## 14. 给实现者的工作方式

1. 先读完整篇规格，再写代码。不要扩展范围。
2. 按阶段 A → B → C → D 推进。每阶段结束应能运行。
3. 如果官方 CLI 的命令与本文假设不一致，以你本机查到的官方文档为准，并在 README 记录实际命令。
4. 所有 git 与进程操作要有错误回传，UI 展示可读的失败原因。
5. 提交信息用中文或英文均可，但代码标识符用英文。
6. 不要引入需要登录本应用云服务的依赖。
7. 像素素材必须是原创或可商用开源许可，并在 README 列出作者与协议。
8. 角色名、叙事文案、军营素材必须走主题包配置，不写死在代码里。
9. 若某功能规格冲突，优先级：安全与数据不丢失 > 流程正确 > 军营表现 > 适配器数量 > 视觉打磨。

---

## 15. 文案与默认语言

- UI 默认提供简体中文和英语。
- 关键按钮："招募名将""新建任务""点将""交付""批准""拒绝""停止""重试"。
- 错误语气要具体，例如"无法启动 Claude Code：未找到 claude 命令"，不要只写"出错了"。

---

## 16. 开放决策（实现前不必再问，按默认执行）

| 问题 | 默认 |
| --- | --- |
| 桌面框架 | Tauri 2 |
| 开源协议 | MIT |
| 第一版 CLI | Claude Code、Codex |
| 是否自动合并 | 否，交付前必须用户确认 |
| 是否自动标记完成 | 否，必须用户确认 |
| 是否内置付费模型 | 否 |
| worktree 位置 | 应用数据目录，不放进仓库 |
| 重启后残留 running 任务 | 标记 `interrupted`，绝不自动重跑；恢复只认用户显式操作（§17.2） |
| 项目名 | PixelCode（包名 `pixelcode`，MIT） |
| 场景主题 | 军营（非办公室），主题包配置化 |
| 角色阵容 | 三国名将 7 人，角色为配置文件，随时可换 |
| 叙事梗 | 用户=刘备；任务卡="锦囊"；方案确认="点将"；导出="战报" |
| 默认执行顺序 | 按依赖调度，Git 大营独立任务最多 3 个并行；非 Git 大营顺序执行 |
| 国际化 | 主题包预留 generic-heroes；v1 仅 three-kingdoms |

如果产品负责人另有指示，以新指示覆盖本表，并改规格版本号。

---

## 17. 运行时健壮性（v1 必须实现）

本节把"跑得稳"变成可验收的硬要求。原则：**任何不确定都保守处理——保留现场、阻塞并交给用户决定，绝不自动重放副作用。**

### 17.1 进程树终止与孤儿回收（P0）

- 每个 Agent/检查子进程必须在独立进程组中启动（Unix `setpgid`/`process_group`；Windows `CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`），`RunSlot::stop/terminate` 对**整个进程组**做两段式终止：先 SIGTERM（Windows `taskkill /T`），超时后 SIGKILL（`taskkill /T /F`）。只杀顶层进程会让 Agent 拉起的 `npm test`、dev server 变成孤儿，禁止。
- 每个子进程带 `PIXELCODE_PARENT=<pid>` 环境标记。应用启动时扫描并回收上次崩溃遗留的带标记孤儿进程（先 SIGTERM 再 SIGKILL）；标记不得出现在用户自己启动的同名 CLI 进程上——只回收进程组归属可判定的本应用后代。

### 17.2 重启恢复与中断语义（P0）

- 任务状态机区分 `interrupted`（用户停止或应用退出导致）与 `failed`（CLI/检查/审查真实失败）。应用启动时把数据库中残留的 `running / checking / reviewing` 任务置为 `interrupted`，计划卡置为 `aborted`；UI 显示"上次运行被中断"并给出恢复入口。
- 恢复只认用户显式操作：用户可对计划卡点"继续"，调度器从最近一个 `done` 任务之后重排剩余 DAG；**绝不自动重跑**，更不自动重放已批准的 git commit/merge。
- worktree 目录名必须由 `plan_id + task_id` 确定性生成，崩溃后可按同名重新认领；发现目录残留但数据库无对应记录时列为"待清理"诊断项展示给用户，不自动强删。
- 集成前校验：应用 patch 前，比较主目录中本任务声明范围内的文件自快照以来是否被外部修改；是则 `blocked` 并保留 worktree 与补丁，不猜测如何合并。

### 17.3 会话续接（P1）

- 适配器从 CLI 输出中提取会话标识并存库（Claude `session_id`、Codex thread/rollout id），随任务记录保存；返工（检查失败、审查打回）与队长澄清续接优先用 CLI 自身的 resume 机制（`claude --resume <id>` / `codex exec resume`），不靠提示词重述全部上下文。
- resume 前先 `--help` 与官方文档确认当前版本参数，参数随 §8.3 版本兼容表维护；探测不到可续接会话时退回全新会话，并在提示词中附上产物引用。
- 评审/审查会话保持全新独立会话（只读边界不变）；不为续接去掉 Codex `--ephemeral` 之外的隔离参数。

### 17.4 锦囊写入范围 `files`（P1）

- 队长计划的每个任务可带 `files`：项目相对路径数组，声明该任务预期写入的文件范围。路径必须是相对路径，拒绝绝对路径、`..`、`\`、`:` 与空值；非法计划在点将前校验拒绝。
- 调度器用 `files` 做事前冲突控制：范围重叠（含目录前缀包含）的任务即使无依赖也串行执行，与 `MAX_PARALLEL` 取交集。
- `files` 是调度提示与集成前校验的依据，不是安全边界；越界写入仍由 worktree/patch 与 §17.5 的规则信封约束。

### 17.5 Worker 规则信封（P1）

- 每个执行角色的运行提示词在任务正文之后追加固定规则块（所有角色共用一份，放 `roles.rs` 或独立配置，不逐角色复制）：只写 `files` 声明范围、不 `git commit/push`、不切分支、不安装依赖、不做全仓格式化；需要超出权限或范围的操作时，以 `BLOCKED: <原因>` 结束并停止，而不是失败硬闯。
- 队长与审查员有各自的只读/规划规则块；审查员保持"最后仅一行 verdict JSON"契约不变。
- 现有 CLI 硬限制（`--disallowedTools`、Codex `--sandbox`）保留，规则信封是叠加层：让 Agent 主动上报"需扩权"成为 `blocked` 状态的一种可读原因，而不是无声失败。

### 17.6 事件与待答持久化（P1）

- 新增 `run_events` 表：`run_id`、`seq`（跑次内递增）、`kind`、`task_id`、`payload_json`、`created_at`；Tauri event 只作"有新事件"的提示，前端按 `seq` 拉取增量，关窗重开可补读。
- 队长澄清的待答（`questions` + `answer`）落库为待答记录（`pending_questions`），带幂等答复键；重复/迟到回答只生效一次，UI 断开不丢问题。
- messages 表继续只存人类可读摘要；`run_events` 存机器可重放的事件流，两者不混用。

### 17.7 事件模型扩展（P2）

- `RunUpdate` 增加 `usage_limited{resets_at}` 与 `turn_metrics{input/output/cache tokens}` 变体；适配器能解析就解析，解析不到就不发，不编造数值。军营场景可显示"伙伴休息到 xx:xx"，战报可统计用量。
- 审批/提问类事件（工具被权限拒绝、Agent 提问）作为 `needs_input` 语义投递给队长或用户展示，不允许静默吞掉。

### 17.8 系统通知（P1）

- 用户可选开启（`app_settings` 开关）：仅在窗口不在前台时，以下事件各发一条系统通知——需要回答、任务被阻塞、计划待交付、运行失败。
- 按持久事件 ID 去重（同一事件不重复弹），点击通知跳回对应跑次/锦囊；窗口可见时静默。通知正文不含提示词与代码内容。

### 17.9 测试策略（P0–P1）

- **P0**：新增一个跨平台伪 CLI 进程测试——真 `spawn` 一个测试夹具程序，走完 stdin 写入、分段 JSONL、stderr、异常退出、stop-before-spawn，断言事件序列与进程树清理。
- **P1**：以固定时钟回放一条"澄清 → 批准 → 检查失败重试 → 审查打回 → 中断/恢复"完整时间线，断言任务状态迁移、产物、worktree 与通知意图；沿用现有 `Fake Runner` + fixture 思路，不依赖真实账号。
- 真实 CLI / 原生 GUI 冒烟标记为 opt-in（本机手动跑），CI 不要求登录账户；Linux 与 Windows 都要过，断言路径分隔符用 `Path` 语义而非 `'/'`。

### 17.10 明确不做

- 不引入 PTY/daemon sidecar/远程同步/多 provider 扩张；"退出窗口后继续跑"若成为需求，另立规格评估本地 sidecar，不在 v1 范围。
- 不自动重试已失败任务、不自动合并、不自动标记完成（与 §7.4、§16 一致）。

---

