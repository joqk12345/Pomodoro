# Pomodoro

Pomodoro 是一个本地优先、任务驱动、带中断处理与 overlearning 语义的 Tauri 桌面番茄工作台，不是普通倒计时器。

## 产品定位

- 先规划，再开始计时
- 一个番茄只服务一个明确任务
- 中断必须显式记录和处理
- 任务提前完成后，剩余时间进入 `overlearning`
- 数据用于复盘，不用于打卡或排名

## 当前实现

当前基线版本：`0.6.1`

已实现的主流程：

- `Today / Focus / Records / Analytics` 四面板结构
- 今日任务新增、编辑、删除、完成/撤销完成、上下移动
- 历史逾期任务查看、移回今天、改期
- 固定节奏 `25 / 5 / 15`
- 同一时刻仅允许一个 `active_timer`
- `focus` 中支持 `pause / resume / complete / abort`
- `break` 中支持 `pause / resume / complete`
- 任务在运行中的 `focus` 可标记完成并进入 `overlearning`
- 中断记录：`internal | external` + `postpone | pause | abort`
- 最近 12 条 session 与 interruption 记录
- 今日概览统计
- 最近 14 天趋势统计
- 完整 JSON 导出
- 完整 JSON 导入
- 兼容旧版导出格式导入
- `active_timer` 和 `cycle_focus_count` 持久化到本地 SQLite

## 技术栈

- React 19
- TypeScript
- Vite
- Tauri 2
- Rust
- SQLite (`rusqlite`)
- `tauri-plugin-dialog`
- `tauri-plugin-notification`

## 项目结构

- [src/App.tsx](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/src/App.tsx)
  主界面、四面板布局、用户交互
- [src/lib/api.ts](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/src/lib/api.ts)
  前端调用 Tauri commands 的封装
- [src/types.ts](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/src/types.ts)
  前端视图模型
- [src-tauri/src/lib.rs](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/src-tauri/src/lib.rs)
  SQLite、核心业务规则、快照聚合、Tauri commands
- [src-tauri/tauri.conf.json](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/src-tauri/tauri.conf.json)
  桌面窗口与构建配置
- [dev-spec.md](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/dev-spec.md)
  当前版本的开发规格基线

## 本地运行

前置要求：

- Node.js 22+
- npm 11+
- Rust / Cargo
- Tauri 2 所需桌面环境依赖

安装依赖：

```bash
npm install
```

启动前端开发服务器：

```bash
npm run dev
```

启动桌面应用开发模式：

```bash
npm run tauri dev
```

构建前端：

```bash
npm run build
```

按当前宿主机或多个本机兼容目标构建桌面包：

```bash
npm run build:multi
```

例如在 macOS 上分别构建 Apple Silicon 与 Intel：

```bash
npm run build:multi -- --preset mac
```

版本同步与发布：

```bash
./scripts/check-version.sh
./scripts/sync-version.sh 0.6.1
./scripts/release.sh 0.6.1
```

GitHub Actions：

- [ci.yml](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/.github/workflows/ci.yml)
  在 `push` / `pull_request` 时执行版本校验、前端构建和 Rust 检查
- [release.yml](/Users/mac/Documents/workspace/Data/01_Work/Projects/books/ai-coding/Pomodoro/.github/workflows/release.yml)
  在 `v*` tag push 或手动触发时构建并发布多平台安装包

检查 Rust / Tauri 后端：

```bash
cd src-tauri
cargo check
```

## 数据存储

应用使用本地 SQLite，数据库文件为：

- `pomodoro.sqlite3`

存储位置为 Tauri `app_data_dir` 下。

主要表结构：

- `tasks`
- `sessions`
- `interruptions`
- `app_state`

其中 `app_state` 当前保存：

- `active_timer`
- `cycle_focus_count`

导入说明：

- 侧边栏的“导入完整 JSON”会覆盖当前本地全部数据
- 当前支持导入本仓库 `0.6.x` 导出的完整 JSON
- 兼容导入旧版顶层 `activeTimer` 结构

## 业务约束

- 任务状态只有 `todo | done`
- 只有 `todo` 任务可以启动 `focus`
- 当前激活任务不能删除、不能改期
- `break` 结束后不会自动开启下一轮 `focus`
- 只有运行中的 `focus` 可以进入 `overlearning`
- interruption 只能挂在当前 `focus` session 上

## 当前限制

- 前端逻辑主要集中在单个 `App.tsx`
- 没有自动化测试
- 没有数据库 migration 体系
- 分析页 `estimate vs actual` 只基于今天可见任务
- 历史列表只展示最近 12 条，不支持分页

## 验证状态

当前仓库已经通过：

- `npm run build`
- `cargo check`
- `cargo test imports_legacy_payload_shape`
- `env POMODORO_IMPORT_FILE=/Users/mac/Downloads/test-podomo.json cargo test imports_external_fixture_when_requested`

## 后续建议

更合理的下一步通常是：

1. 拆分前端模块与状态
2. 引入数据库 migration
3. 增加测试
4. 扩展任务维度与历史筛选
