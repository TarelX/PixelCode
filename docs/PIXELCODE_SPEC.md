# PixelCode 开发规格

版本：0.4  
日期：2026-11-21（Asia/Singapore，UTC+8）  
状态：可直接按本文实现 v1  
本文是给实现者的开发规格，不是营销文案。实现时以本文为准；本文未写的功能一律不做。

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
| 锦囊（Task Card） | 队长拆解出的一个具体任务，写明角色、依赖、输入、输出、验收标准、检查命令。 |
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
调度器按依赖执行任务（v1 先顺序，v2 加并行）
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
- `tasks`：锦囊列表，每个锦囊写明负责名将、依赖、输入产物、输出产物、验收标准、检查命令

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

启动时和进入设置时扫描 `PATH`：
- 检查 CLI 是否存在。
- 检查是否已登录（用无害命令，如 `claude auth status`、`codex --version`）。
- 显示"已找到 / 未找到 / 已找到但未登录"。

未找到任何 CLI 时，打开大营后界面可用，但新建任务会被拦截，并给出安装文档链接。

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
- 进程管理：Rust 侧管理 CLI 进程（`portable-pty` 或等价库），通过 Tauri event 把输出流到前端
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
│  - AgentSupervisor (spawn/pty/kill)     │
│  - AdapterRegistry (CLI 适配器)          │
│  - GitService (worktree/branch/diff)    │
└──────────────▲──────────────────────────┘
               │ child process, cwd=worktree
┌──────────────▼──────────────────────────┐
│  用户已安装的 CLI                       │
│  claude / codex                         │
└─────────────────────────────────────────┘
```

### 9.3 适配器接口

```ts
interface AgentAdapter {
  id: "claude-code" | "codex";
  displayName: string;
  detect(): Promise<DetectResult>;
  start(opts: StartOptions): Promise<void>; // cwd, cols, rows, systemPrompt, skills, permissions
  write(sessionId: string, input: string): Promise<void>;
  approve(sessionId: string, decision: "approve" | "reject"): Promise<void>;
  resize(sessionId: string, cols: number, rows: number): Promise<void>;
  stop(sessionId: string): Promise<void>;
}
```

每个适配器单独一个文件，禁止把所有 CLI 的参数解析写在一个巨大 switch 里。

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
- `status` TEXT NOT NULL（pending, running, checking, reviewing, done, blocked, failed）
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
| 项目名 | PixelCode（包名 `pixelcode`，MIT） |
| 场景主题 | 军营（非办公室），主题包配置化 |
| 角色阵容 | 三国名将 7 人，角色为配置文件，随时可换 |
| 叙事梗 | 用户=刘备；任务卡="锦囊"；方案确认="点将"；导出="战报" |
| 默认执行顺序 | 顺序执行，v2 再加并行 |
| 国际化 | 主题包预留 generic-heroes；v1 仅 three-kingdoms |

如果产品负责人另有指示，以新指示覆盖本表，并改规格版本号。
