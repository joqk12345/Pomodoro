# Pomodoro Dev Spec

## 1. 文档目的

本说明基于当前仓库代码整理，不是概念提案，而是现有应用的实现基线。后续如果要继续生成、重构或扩展应用，默认以这里的产品约束、数据模型和行为规则为准，除非明确提出变更。

当前基线版本：`0.5.0`

## 2. 产品定义

Pomodoro 不是“一个简单倒计时器”，而是一个面向个人知识工作者的本地桌面工作台。它强调：

- 先规划，再开始计时
- 一个番茄只服务一个明确任务
- 中断必须被显式记录和处理
- 任务提前完成后，剩余时间进入 `overlearning`
- 数据用于复盘，不用于打卡或排名

## 3. 目标用户

- 单机使用的个人用户
- 需要用番茄工作法管理日常知识工作的人
- 不依赖团队协作、账号体系、云同步

## 4. 当前功能范围

### 4.1 任务规划

- 维护今天的任务列表 `To Do Today`
- 任务字段包含：标题、优先级、预估番茄数、备注、计划日期
- 支持任务新增、编辑、删除、完成/撤销完成、今日队列上下移动
- 支持查看历史未完成任务 `Overdue tasks`
- 逾期任务可直接移动回今天，或改期到其他日期

### 4.2 专注流程

- 只有 `todo` 状态任务可以启动专注
- 专注节奏固定为 `25 / 5 / 15`
  - focus: 25 分钟
  - short break: 5 分钟
  - long break: 15 分钟
- 完成 4 个 focus 后，下一次休息进入 long break
- focus 中允许：
  - pause / resume
  - 手动结束当前 phase
  - abort 当前番茄
  - 将当前任务标记完成并进入 `overlearning`
- break 中允许：
  - pause / resume
  - 手动结束当前 phase
  - skip break

### 4.3 中断处理

- 中断只能挂在当前 `focus` session 上
- 中断字段包含：
  - source: `internal | external`
  - note
  - resolution: `postpone | pause | abort`
- `pause` 会暂停当前 timer
- `abort` 会结束当前 focus session 并标记为 `aborted`
- `postpone` 只记录，不改变当前 timer

### 4.4 记录与分析

- 展示最近 12 条 sessions
- 展示最近 12 条 interruptions
- 展示今天概览指标：
  - 完成番茄数
  - 作废番茄数
  - 专注分钟数
  - overlearning 分钟数
  - 今日完成任务数
  - 今日中断数
- 生成最近 14 天趋势统计
- 提供：
  - 折线图：每日 focus 分钟
  - 柱状图：最近 7 天 completed vs aborted
  - 今日 vs 昨日对比
  - 本周 vs 上周对比
  - 当日任务预估 vs 实际偏差

### 4.5 导出与提醒

- 支持导出完整 JSON 数据
- focus 结束、break 结束会触发：
  - 应用内阶段弹窗
  - 系统通知
  - 浏览器音频提醒

## 5. 非目标

当前版本明确不包含：

- 用户注册、登录、账号体系
- 云同步
- 多设备同步
- 团队任务协作
- 标签、项目、子任务、多级看板
- 自定义番茄时长
- 深度报表、筛选器、搜索系统
- 数据库迁移框架
- 自动化测试体系

## 6. 信息架构

前端固定为四个主面板：

- `Today`
  - 新增任务
  - 逾期任务
  - 今日待办
  - 今日已完成
- `Focus`
  - 当前 timer
  - 阶段进度条
  - timer 控制按钮
  - interruption handling
  - 最近中断
- `Records`
  - session history
  - interruption log
- `Analytics`
  - 14 天趋势
  - 日对比
  - 周对比
  - estimate audit

页面原则：规划、执行、记录、分析分离，不做“大杂烩式 dashboard”。

## 7. 核心业务规则

### 7.1 任务规则

- 任务状态仅有：`todo | done`
- 新任务默认进入今天
- 优先级范围：`1..5`
- 预估番茄数范围：`1..12`
- 标题不能为空
- 任务支持改期，但当前激活中的任务不能改期
- 当前激活中的任务不能删除

### 7.2 计时规则

- 同一时刻最多只有一个 `active_timer`
- `active_timer` 保存在持久化状态中，应用重启后仍可恢复
- timer 状态仅有：`running | paused`
- focus session 结束后自动生成 break timer
- break 结束后不会自动开启下一轮 focus，用户必须重新选择任务

### 7.3 Overlearning 规则

- 仅 focus session 可进入 overlearning
- 必须在 timer 运行中才能把任务标记为完成
- 标记完成后：
  - 任务状态变为 `done`
  - 当前番茄不会立刻结束
  - 剩余时间转为 `overlearning`
- overlearning 时长单独统计

### 7.4 周期规则

- 每完成一个 focus，`cycle_focus_count + 1`
- 当累计达到 4 时：
  - 下一阶段为 `long_break`
  - break 结束后计数重置为 0
- 其他情况下，下一阶段为 `short_break`

## 8. 数据模型

### 8.1 Task

```ts
{
  id: string
  title: string
  priority: number
  estimatedPomodoros: number
  actualPomodoros: number
  status: 'todo' | 'done'
  todayOrder: number
  notes: string
  scheduledDate: 'YYYY-MM-DD'
}
```

说明：

- `actualPomodoros` 不是直接存字段，而是由已完成 focus sessions 聚合得出
- `todayOrder` 用于同一天任务排序

### 8.2 ActiveTimer

```ts
{
  sessionId: string
  taskId: string | null
  taskTitle: string | null
  phaseType: 'focus' | 'short_break' | 'long_break'
  status: 'running' | 'paused'
  startedAt: string
  endsAt: string
  remainingSeconds: number
  plannedSeconds: number
  pomodoroIndex: number
  overlearningStartedAt: string | null
}
```

### 8.3 Session

```ts
{
  id: string
  phaseType: 'focus' | 'short_break' | 'long_break'
  status: 'active' | 'paused' | 'completed' | 'aborted'
  taskTitle: string | null
  startedAt: string
  endedAt: string | null
  focusSeconds: number
  overlearningSeconds: number
  pausedSeconds: number
  pomodoroIndex: number
}
```

### 8.4 Interruption

```ts
{
  id: string
  taskTitle: string | null
  source: 'internal' | 'external'
  note: string
  resolution: 'postpone' | 'pause' | 'abort'
  createdAt: string
}
```

## 9. 存储设计

当前使用本地 SQLite，文件位置为 Tauri `app_data_dir` 下：

- `pomodoro.sqlite3`

数据库表：

- `tasks`
- `sessions`
- `interruptions`
- `app_state`

### 9.1 tasks

- 持久化任务基础信息与计划日期
- `completed_at` 仅在任务完成时写入

### 9.2 sessions

- 每个 focus / break 都是一条 session
- `day_key` 用于日统计
- `focus_seconds / overlearning_seconds / paused_seconds` 在 session 收尾时计算落库

### 9.3 interruptions

- 每次中断单独一条记录
- 必须关联到某个 session

### 9.4 app_state

当前承载两个系统级状态：

- `active_timer`
- `cycle_focus_count`

其中 `active_timer` 以 JSON 序列化方式存储。

## 10. 前后端契约

前端通过 Tauri `invoke` 调用 Rust 命令。

### 10.1 Query

- `get_app_snapshot`
  - 返回首页所需完整快照
- `export_data`
  - 返回完整导出 JSON 字符串

### 10.2 Task commands

- `create_task(input)`
- `update_task(taskId, input)`
- `move_task(taskId, direction)`
- `delete_task(taskId)`
- `toggle_task_completion(taskId, completed)`

### 10.3 Timer commands

- `start_focus_session(taskId)`
- `mark_active_task_completed()`
- `pause_active_timer()`
- `resume_active_timer()`
- `abort_active_timer()`
- `complete_active_timer()`

### 10.4 Interruption command

- `record_interruption(input)`

## 11. Snapshot 视图模型

前端主视图依赖单个 `AppSnapshot`：

```ts
{
  tasks: TaskView[]
  overdueTasks: TaskView[]
  activeTimer: ActiveTimerView | null
  recentSessions: SessionView[]
  recentInterruptions: InterruptionView[]
  overview: OverviewStats
  dailyStats: DailyStat[]
}
```

设计意图：

- 前端尽量不自行拼接复杂业务数据
- Rust 负责聚合与裁剪
- UI 刷新通过重新拉取完整 snapshot 完成

## 12. 技术架构

### 12.1 前端

- React 19
- TypeScript
- Vite
- 单页应用
- 主要状态集中在 `src/App.tsx`

### 12.2 桌面壳

- Tauri 2
- 单窗口桌面应用
- 默认窗口尺寸：`1440 x 920`
- 最小尺寸：`1180 x 780`

### 12.3 后端

- Rust
- `rusqlite` 本地数据库
- `chrono` 处理时间
- `uuid` 生成 ID
- `tauri-plugin-notification` 提供系统通知

## 13. 时间与日期约定

- `scheduledDate` / `day_key` 格式：`YYYY-MM-DD`
- 时间戳统一为 ISO 8601 字符串
- 日统计按本地日期 `Local::now()` 计算
- 时间持久化以 UTC ISO 字符串为主

这意味着后续扩展时必须谨慎处理：

- 本地日界线
- 时区切换
- 跨天中的 active session

## 14. 当前实现特征与限制

- 前端主要逻辑集中在单个 `App.tsx`，尚未做模块化拆分
- 当前没有测试代码
- 当前没有数据库 migration 版本管理
- `Today` 页新建任务固定写入“今天”，不是任意日期创建
- 分析页里的 `estimate vs actual` 只基于今天可见任务，不是全量历史任务
- 历史列表只展示最近 12 条，不支持分页
- 所有统计围绕个人本地使用，不考虑多人数据隔离

## 15. 生成后续版本时的硬约束

如果后续基于该项目继续生成应用，默认保留以下约束：

- 仍是本地优先的桌面应用
- 不引入账号和云同步，除非明确要求
- 仍保持 `Today / Focus / Records / Analytics` 四段式结构
- 仍保持“一个 active timer + 一个 active task”的执行模型
- 仍保留 interruption 显式记录机制
- 仍保留 overlearning 语义，而不是任务完成即立刻结束 session
- 仍以 Rust 聚合 snapshot，前端消费视图模型

## 16. 可以优先扩展的方向

如果后面要继续生成应用，最合理的扩展顺序是：

1. 拆分前端模块与状态管理
2. 引入数据库迁移机制
3. 增加测试
4. 扩展任务维度，例如项目、标签、归档
5. 增加更完整的历史筛选与统计
6. 再考虑同步、备份或跨端能力

## 17. 仓库关键文件

- `src/App.tsx`
  - 主界面、面板切换、前端交互
- `src/lib/api.ts`
  - Tauri invoke 封装
- `src/types.ts`
  - 前端视图模型定义
- `src-tauri/src/lib.rs`
  - 核心业务逻辑、SQLite 访问、Tauri commands
- `src-tauri/tauri.conf.json`
  - 桌面应用配置
- `README.md`
  - 产品定位与使用说明

## 18. 一句话规格总结

Pomodoro 当前版本的本质是：一个本地优先、任务驱动、带中断处理与 overlearning 语义的 Tauri 桌面番茄工作台，而不是普通倒计时器。
