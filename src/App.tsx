import { useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  abortActiveTimer,
  cloneTaskToInput,
  completeActiveTimer,
  createTask,
  deleteTask,
  exportData,
  getAppSnapshot,
  importDataFromPath,
  markActiveTaskCompleted,
  moveTask,
  pauseActiveTimer,
  recordInterruption,
  resumeActiveTimer,
  startFocusSession,
  toggleTaskCompletion,
  updateTask,
} from './lib/api';
import type {
  ActiveTimerView,
  AppSnapshot,
  DailyStat,
  PanelKey,
  RecordInterruptionInput,
  TaskInput,
  TaskView,
} from './types';

const PANELS: PanelKey[] = ['Today', 'Focus', 'Records', 'Analytics'];

const PHASE_LABELS: Record<ActiveTimerView['phaseType'], string> = {
  focus: 'Focus',
  short_break: 'Short Break',
  long_break: 'Long Break',
};

const STATUS_LABELS: Record<string, string> = {
  active: '进行中',
  paused: '暂停中',
  completed: '已完成',
  aborted: '已作废',
};

const todayInput = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = `${now.getMonth() + 1}`.padStart(2, '0');
  const day = `${now.getDate()}`.padStart(2, '0');
  return `${year}-${month}-${day}`;
};

const formatDateTime = (value: string | null | undefined) => {
  if (!value) {
    return '—';
  }

  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
};

const formatMinutes = (seconds: number) => `${Math.floor(seconds / 60)} min`;

const formatTimer = (seconds: number) => {
  const safeSeconds = Math.max(0, seconds);
  const minutes = Math.floor(safeSeconds / 60)
    .toString()
    .padStart(2, '0');
  const remainSeconds = (safeSeconds % 60).toString().padStart(2, '0');
  return `${minutes}:${remainSeconds}`;
};

const getRemainingSeconds = (timer: ActiveTimerView, now: number) => {
  if (timer.status === 'paused') {
    return timer.remainingSeconds;
  }

  const remaining = Math.ceil(
    (new Date(timer.endsAt).getTime() - now) / 1000,
  );
  return Math.max(0, remaining);
};

const getProgressRatio = (timer: ActiveTimerView, now: number) => {
  const remaining = getRemainingSeconds(timer, now);
  return Math.max(
    0,
    Math.min(1, (timer.plannedSeconds - remaining) / timer.plannedSeconds),
  );
};

const downloadText = (filename: string, content: string) => {
  const blob = new Blob([content], { type: 'application/json;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
};

const playAlertTone = () => {
  const AudioCtor = window.AudioContext as
    | typeof AudioContext
    | undefined;

  if (!AudioCtor) {
    return;
  }

  const ctx = new AudioCtor();
  const oscillator = ctx.createOscillator();
  const gain = ctx.createGain();
  oscillator.type = 'triangle';
  oscillator.frequency.value = 740;
  gain.gain.value = 0.04;
  oscillator.connect(gain);
  gain.connect(ctx.destination);
  oscillator.start();
  oscillator.stop(ctx.currentTime + 0.18);
  oscillator.onended = () => {
    void ctx.close();
  };
};

const byDayKey = (stats: DailyStat[]) => {
  const bucket = new Map<string, DailyStat>();
  for (const stat of stats) {
    bucket.set(stat.dayKey, stat);
  }
  return bucket;
};

const isoDay = (date: Date) => {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, '0');
  const day = `${date.getDate()}`.padStart(2, '0');
  return `${year}-${month}-${day}`;
};

const mondayStart = (date: Date) => {
  const copy = new Date(date);
  copy.setHours(0, 0, 0, 0);
  const day = copy.getDay();
  const offset = day === 0 ? 6 : day - 1;
  copy.setDate(copy.getDate() - offset);
  return copy;
};

const sumFocusInRange = (stats: DailyStat[], start: Date, days: number) => {
  const lookup = byDayKey(stats);
  let focus = 0;
  let completed = 0;
  let aborted = 0;

  for (let index = 0; index < days; index += 1) {
    const current = new Date(start);
    current.setDate(start.getDate() + index);
    const stat = lookup.get(isoDay(current));
    if (!stat) {
      continue;
    }
    focus += stat.focusMinutes;
    completed += stat.completedPomodoros;
    aborted += stat.abortedPomodoros;
  }

  return { focus, completed, aborted };
};

const TrendLine = ({ stats }: { stats: DailyStat[] }) => {
  const max = Math.max(1, ...stats.map((item) => item.focusMinutes));
  const points = stats
    .map((item, index) => {
      const x = (index / Math.max(1, stats.length - 1)) * 100;
      const y = 100 - (item.focusMinutes / max) * 100;
      return `${x},${y}`;
    })
    .join(' ');

  return (
    <div className="chart-shell">
      <svg viewBox="0 0 100 100" preserveAspectRatio="none">
        <defs>
          <linearGradient id="trendGradient" x1="0%" x2="0%" y1="0%" y2="100%">
            <stop offset="0%" stopColor="rgba(252, 110, 81, 0.9)" />
            <stop offset="100%" stopColor="rgba(252, 110, 81, 0.1)" />
          </linearGradient>
        </defs>
        <polyline
          fill="none"
          points={points}
          stroke="rgba(252, 110, 81, 0.95)"
          strokeWidth="2.5"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <div className="chart-foot">
        {stats.map((item) => (
          <span key={item.dayKey}>{item.dayKey.slice(5)}</span>
        ))}
      </div>
    </div>
  );
};

const CompareBars = ({ stats }: { stats: DailyStat[] }) => {
  const recent = stats.slice(-7);
  const max = Math.max(
    1,
    ...recent.flatMap((item) => [item.completedPomodoros, item.abortedPomodoros]),
  );

  return (
    <div className="bars-grid">
      {recent.map((item) => (
        <div className="bar-card" key={item.dayKey}>
          <div className="bar-stack">
            <span
              className="bar completed"
              style={{ height: `${(item.completedPomodoros / max) * 100}%` }}
            />
            <span
              className="bar aborted"
              style={{ height: `${(item.abortedPomodoros / max) * 100}%` }}
            />
          </div>
          <div className="bar-label">{item.dayKey.slice(5)}</div>
        </div>
      ))}
    </div>
  );
};

export default function App() {
  const [panel, setPanel] = useState<PanelKey>('Today');
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const [phaseAlert, setPhaseAlert] = useState<string | null>(null);
  const [newTask, setNewTask] = useState<TaskInput>({
    title: '',
    priority: 3,
    estimatedPomodoros: 1,
    notes: '',
  });
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTask, setEditingTask] = useState<TaskInput | null>(null);
  const [reschedules, setReschedules] = useState<Record<string, string>>({});
  const [interruption, setInterruption] = useState<RecordInterruptionInput>({
    source: 'external',
    note: '',
    resolution: 'postpone',
  });
  const initializedRef = useRef(false);
  const lastTimerRef = useRef<string | null>(null);
  const zeroSyncRef = useRef(false);

  const loadSnapshot = async (keepLoading = false) => {
    try {
      const data = await getAppSnapshot();
      setSnapshot(data);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      if (!keepLoading) {
        setLoading(false);
      }
    }
  };

  const runMutation = async (label: string, operation: () => Promise<void>) => {
    setBusy(label);
    setError(null);
    try {
      await operation();
      await loadSnapshot(true);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  };

  useEffect(() => {
    void loadSnapshot();
  }, []);

  useEffect(() => {
    const tick = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);

    const refresh = window.setInterval(() => {
      void loadSnapshot(true);
    }, 15000);

    return () => {
      window.clearInterval(tick);
      window.clearInterval(refresh);
    };
  }, []);

  useEffect(() => {
    const activeTimer = snapshot?.activeTimer;
    if (!activeTimer) {
      zeroSyncRef.current = false;
      return;
    }

    const remaining = getRemainingSeconds(activeTimer, now);
    if (activeTimer.status === 'running' && remaining === 0) {
      if (!zeroSyncRef.current) {
        zeroSyncRef.current = true;
        void loadSnapshot(true);
      }
    } else {
      zeroSyncRef.current = false;
    }
  }, [now, snapshot]);

  useEffect(() => {
    const timer = snapshot?.activeTimer;
    const signature = timer ? `${timer.sessionId}:${timer.phaseType}` : null;

    if (!initializedRef.current) {
      initializedRef.current = true;
      lastTimerRef.current = signature;
      return;
    }

    if (lastTimerRef.current && signature !== lastTimerRef.current) {
      const message = timer
        ? `${PHASE_LABELS[timer.phaseType]} 已开始`
        : '当前阶段已结束';
      setPhaseAlert(message);
      playAlertTone();
      window.setTimeout(() => setPhaseAlert(null), 4000);
    }

    lastTimerRef.current = signature;
  }, [snapshot?.activeTimer]);

  if (loading && !snapshot) {
    return <div className="splash">Loading Pomodoro workbench…</div>;
  }

  const data = snapshot;
  const todayTasks = data?.tasks ?? [];
  const todoTasks = todayTasks.filter((task) => task.status === 'todo');
  const doneTasks = todayTasks.filter((task) => task.status === 'done');
  const activeTimer = data?.activeTimer ?? null;
  const remainingSeconds = activeTimer
    ? getRemainingSeconds(activeTimer, now)
    : 0;
  const progressRatio = activeTimer ? getProgressRatio(activeTimer, now) : 0;

  const today = todayInput();
  const todayStat = data?.dailyStats[data.dailyStats.length - 1];
  const yesterdayStat = data?.dailyStats[data.dailyStats.length - 2];
  const thisWeek = data
    ? sumFocusInRange(data.dailyStats, mondayStart(new Date()), 7)
    : { focus: 0, completed: 0, aborted: 0 };
  const previousWeekStart = mondayStart(new Date());
  previousWeekStart.setDate(previousWeekStart.getDate() - 7);
  const lastWeek = data
    ? sumFocusInRange(data.dailyStats, previousWeekStart, 7)
    : { focus: 0, completed: 0, aborted: 0 };

  const handleExport = async () => {
    setBusy('export');
    setError(null);
    try {
      const content = await exportData();
      downloadText(`pomodoro-export-${today}.json`, content);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  };

  const handleImport = async () => {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        filters: [
          {
            name: 'JSON',
            extensions: ['json'],
          },
        ],
      });

      if (!selected || Array.isArray(selected)) {
        return;
      }

      const confirmed = window.confirm(
        '导入会覆盖当前本地全部任务、记录和计时状态，是否继续？',
      );
      if (!confirmed) {
        return;
      }

      await runMutation('import-data', () => importDataFromPath(selected));
      setPhaseAlert('导入完成');
      window.setTimeout(() => setPhaseAlert(null), 4000);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const beginEdit = (task: TaskView) => {
    setEditingId(task.id);
    setEditingTask(cloneTaskToInput(task));
  };

  return (
    <div className="app-shell">
      <div className="backdrop" />
      <aside className="sidebar">
        <div>
          <p className="eyebrow">Pomodoro 0.6.1</p>
          <h1>Work the plan. Then count the cycle.</h1>
          <p className="sidebar-copy">
            一个本地优先、任务驱动、显式处理中断与 overlearning 的桌面番茄工作台。
          </p>
        </div>
        <nav className="nav-list">
          {PANELS.map((item) => (
            <button
              className={item === panel ? 'nav-button active' : 'nav-button'}
              key={item}
              onClick={() => setPanel(item)}
              type="button"
            >
              <span>{item}</span>
              <small>
                {item === 'Today' && `${todoTasks.length} todo`}
                {item === 'Focus' && (activeTimer ? PHASE_LABELS[activeTimer.phaseType] : 'Idle')}
                {item === 'Records' &&
                  `${data?.recentSessions.length ?? 0} recent`}
                {item === 'Analytics' &&
                  `${data?.overview.focusMinutes ?? 0} min today`}
              </small>
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <button
            className="ghost-button"
            disabled={busy !== null}
            onClick={() => {
              void handleImport();
            }}
            type="button"
          >
            导入完整 JSON
          </button>
          <button
            className="ghost-button"
            disabled={busy !== null}
            onClick={handleExport}
            type="button"
          >
            导出完整 JSON
          </button>
        </div>
      </aside>

      <main className="content">
        <header className="topbar">
          <div>
            <p className="eyebrow">Today Snapshot</p>
            <h2>
              {data?.overview.completedPomodoros ?? 0} completed /{' '}
              {data?.overview.abortedPomodoros ?? 0} aborted
            </h2>
          </div>
          <div className="topbar-actions">
            <div className="metric-chip">
              <span>Focus</span>
              <strong>{data?.overview.focusMinutes ?? 0} min</strong>
            </div>
            <div className="metric-chip">
              <span>Overlearning</span>
              <strong>{data?.overview.overlearningMinutes ?? 0} min</strong>
            </div>
            <div className="metric-chip">
              <span>Interruptions</span>
              <strong>{data?.overview.interruptionsToday ?? 0}</strong>
            </div>
          </div>
        </header>

        {error ? <div className="error-banner">{error}</div> : null}
        {phaseAlert ? <div className="phase-alert">{phaseAlert}</div> : null}

        {panel === 'Today' ? (
          <section className="panel-grid">
            <article className="card stretch">
              <div className="card-header">
                <div>
                  <p className="eyebrow">New Task</p>
                  <h3>To Do Today</h3>
                </div>
              </div>
              <form
                className="task-form"
                onSubmit={(event) => {
                  event.preventDefault();
                  void runMutation('create-task', async () => {
                    await createTask(newTask);
                    setNewTask({
                      title: '',
                      priority: 3,
                      estimatedPomodoros: 1,
                      notes: '',
                    });
                  });
                }}
              >
                <input
                  placeholder="任务标题"
                  value={newTask.title}
                  onChange={(event) =>
                    setNewTask((current) => ({
                      ...current,
                      title: event.target.value,
                    }))
                  }
                />
                <div className="inline-fields">
                  <label>
                    Priority
                    <input
                      max={5}
                      min={1}
                      type="number"
                      value={newTask.priority}
                      onChange={(event) =>
                        setNewTask((current) => ({
                          ...current,
                          priority: Number(event.target.value),
                        }))
                      }
                    />
                  </label>
                  <label>
                    Estimate
                    <input
                      max={12}
                      min={1}
                      type="number"
                      value={newTask.estimatedPomodoros}
                      onChange={(event) =>
                        setNewTask((current) => ({
                          ...current,
                          estimatedPomodoros: Number(event.target.value),
                        }))
                      }
                    />
                  </label>
                </div>
                <textarea
                  placeholder="备注"
                  value={newTask.notes}
                  onChange={(event) =>
                    setNewTask((current) => ({
                      ...current,
                      notes: event.target.value,
                    }))
                  }
                />
                <button className="primary-button" disabled={busy !== null} type="submit">
                  添加任务
                </button>
              </form>
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Overdue</p>
                  <h3>逾期任务</h3>
                </div>
              </div>
              <div className="stack">
                {data?.overdueTasks.length ? (
                  data.overdueTasks.map((task) => (
                    <div className="task-row compact" key={task.id}>
                      <div>
                        <strong>{task.title}</strong>
                        <p>
                          {task.scheduledDate} · P{task.priority} · 估算{' '}
                          {task.estimatedPomodoros}
                        </p>
                      </div>
                      <div className="row-actions">
                        <input
                          type="date"
                          value={reschedules[task.id] ?? today}
                          onChange={(event) =>
                            setReschedules((current) => ({
                              ...current,
                              [task.id]: event.target.value,
                            }))
                          }
                        />
                        <button
                          className="secondary-button"
                          disabled={busy !== null}
                          onClick={() =>
                            void runMutation('move-overdue-today', () =>
                              updateTask(task.id, {
                                ...cloneTaskToInput(task),
                                scheduledDate: today,
                              }),
                            )
                          }
                          type="button"
                        >
                          移回今天
                        </button>
                        <button
                          className="ghost-button"
                          disabled={busy !== null}
                          onClick={() =>
                            void runMutation('reschedule-task', () =>
                              updateTask(task.id, {
                                ...cloneTaskToInput(task),
                                scheduledDate: reschedules[task.id] ?? today,
                              }),
                            )
                          }
                          type="button"
                        >
                          改期
                        </button>
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="empty-state">没有逾期任务。</div>
                )}
              </div>
            </article>

            <article className="card stretch">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Queue</p>
                  <h3>今日待办</h3>
                </div>
              </div>
              <div className="stack">
                {todoTasks.length ? (
                  todoTasks.map((task) => (
                    <div className="task-row" key={task.id}>
                      <div className="task-main">
                        <div className="task-title">
                          <strong>{task.title}</strong>
                          <span className="pill">P{task.priority}</span>
                          <span className="pill subtle">
                            {task.actualPomodoros}/{task.estimatedPomodoros}
                          </span>
                        </div>
                        <p>{task.notes || '无备注'}</p>
                      </div>
                      <div className="row-actions">
                        <button
                          className="primary-button"
                          disabled={busy !== null || activeTimer !== null}
                          onClick={() =>
                            void runMutation('start-focus', () =>
                              startFocusSession(task.id),
                            )
                          }
                          type="button"
                        >
                          开始专注
                        </button>
                        <button
                          className="ghost-button"
                          disabled={busy !== null}
                          onClick={() => void runMutation('move-up', () => moveTask(task.id, 'up'))}
                          type="button"
                        >
                          上移
                        </button>
                        <button
                          className="ghost-button"
                          disabled={busy !== null}
                          onClick={() =>
                            void runMutation('move-down', () => moveTask(task.id, 'down'))
                          }
                          type="button"
                        >
                          下移
                        </button>
                        <button
                          className="ghost-button"
                          disabled={busy !== null}
                          onClick={() => beginEdit(task)}
                          type="button"
                        >
                          编辑
                        </button>
                        <button
                          className="ghost-button"
                          disabled={busy !== null}
                          onClick={() =>
                            void runMutation('complete-task', () =>
                              toggleTaskCompletion(task.id, true),
                            )
                          }
                          type="button"
                        >
                          完成
                        </button>
                        <button
                          className="danger-button"
                          disabled={busy !== null}
                          onClick={() =>
                            void runMutation('delete-task', () => deleteTask(task.id))
                          }
                          type="button"
                        >
                          删除
                        </button>
                      </div>
                      {editingId === task.id && editingTask ? (
                        <form
                          className="editor-panel"
                          onSubmit={(event) => {
                            event.preventDefault();
                            void runMutation('update-task', async () => {
                              await updateTask(task.id, editingTask);
                              setEditingId(null);
                              setEditingTask(null);
                            });
                          }}
                        >
                          <input
                            value={editingTask.title}
                            onChange={(event) =>
                              setEditingTask((current) =>
                                current
                                  ? { ...current, title: event.target.value }
                                  : current,
                              )
                            }
                          />
                          <div className="inline-fields">
                            <label>
                              Priority
                              <input
                                max={5}
                                min={1}
                                type="number"
                                value={editingTask.priority}
                                onChange={(event) =>
                                  setEditingTask((current) =>
                                    current
                                      ? {
                                          ...current,
                                          priority: Number(event.target.value),
                                        }
                                      : current,
                                  )
                                }
                              />
                            </label>
                            <label>
                              Estimate
                              <input
                                max={12}
                                min={1}
                                type="number"
                                value={editingTask.estimatedPomodoros}
                                onChange={(event) =>
                                  setEditingTask((current) =>
                                    current
                                      ? {
                                          ...current,
                                          estimatedPomodoros: Number(
                                            event.target.value,
                                          ),
                                        }
                                      : current,
                                  )
                                }
                              />
                            </label>
                            <label>
                              Schedule
                              <input
                                type="date"
                                value={editingTask.scheduledDate ?? today}
                                onChange={(event) =>
                                  setEditingTask((current) =>
                                    current
                                      ? {
                                          ...current,
                                          scheduledDate: event.target.value,
                                        }
                                      : current,
                                  )
                                }
                              />
                            </label>
                          </div>
                          <textarea
                            value={editingTask.notes}
                            onChange={(event) =>
                              setEditingTask((current) =>
                                current
                                  ? { ...current, notes: event.target.value }
                                  : current,
                              )
                            }
                          />
                          <div className="row-actions">
                            <button
                              className="primary-button"
                              disabled={busy !== null}
                              type="submit"
                            >
                              保存
                            </button>
                            <button
                              className="ghost-button"
                              onClick={() => {
                                setEditingId(null);
                                setEditingTask(null);
                              }}
                              type="button"
                            >
                              取消
                            </button>
                          </div>
                        </form>
                      ) : null}
                    </div>
                  ))
                ) : (
                  <div className="empty-state">今天还没有待办任务。</div>
                )}
              </div>
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Done</p>
                  <h3>今日已完成</h3>
                </div>
              </div>
              <div className="stack">
                {doneTasks.length ? (
                  doneTasks.map((task) => (
                    <div className="task-row compact" key={task.id}>
                      <div>
                        <strong>{task.title}</strong>
                        <p>
                          实际 {task.actualPomodoros} / 预估 {task.estimatedPomodoros}
                        </p>
                      </div>
                      <button
                        className="ghost-button"
                        disabled={busy !== null}
                        onClick={() =>
                          void runMutation('undo-task', () =>
                            toggleTaskCompletion(task.id, false),
                          )
                        }
                        type="button"
                      >
                        撤销完成
                      </button>
                    </div>
                  ))
                ) : (
                  <div className="empty-state">今天还没有完成项。</div>
                )}
              </div>
            </article>
          </section>
        ) : null}

        {panel === 'Focus' ? (
          <section className="panel-grid focus-grid">
            <article className="card hero-card stretch">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Current Phase</p>
                  <h3>{activeTimer ? PHASE_LABELS[activeTimer.phaseType] : 'Idle'}</h3>
                </div>
                <span className="session-tag">
                  {activeTimer ? `Pomodoro ${activeTimer.pomodoroIndex}` : 'No timer'}
                </span>
              </div>
              {activeTimer ? (
                <>
                  <div className="timer-face">{formatTimer(remainingSeconds)}</div>
                  <div className="hero-meta">
                    <strong>{activeTimer.taskTitle ?? 'Break phase'}</strong>
                    <span>
                      {activeTimer.status === 'paused' ? '暂停中' : '运行中'}
                    </span>
                  </div>
                  <div className="progress-rail">
                    <span
                      className="progress-fill"
                      style={{ width: `${progressRatio * 100}%` }}
                    />
                  </div>
                  <div className="row-actions wide">
                    {activeTimer.status === 'running' ? (
                      <button
                        className="secondary-button"
                        disabled={busy !== null}
                        onClick={() => void runMutation('pause-timer', pauseActiveTimer)}
                        type="button"
                      >
                        暂停
                      </button>
                    ) : (
                      <button
                        className="secondary-button"
                        disabled={busy !== null}
                        onClick={() => void runMutation('resume-timer', resumeActiveTimer)}
                        type="button"
                      >
                        恢复
                      </button>
                    )}
                    <button
                      className="primary-button"
                      disabled={busy !== null}
                      onClick={() =>
                        void runMutation('complete-phase', completeActiveTimer)
                      }
                      type="button"
                    >
                      结束当前阶段
                    </button>
                    {activeTimer.phaseType === 'focus' ? (
                      <button
                        className="danger-button"
                        disabled={busy !== null}
                        onClick={() => void runMutation('abort-focus', abortActiveTimer)}
                        type="button"
                      >
                        作废当前番茄
                      </button>
                    ) : null}
                    {activeTimer.phaseType === 'focus' &&
                    activeTimer.taskId &&
                    activeTimer.status === 'running' &&
                    !activeTimer.overlearningStartedAt ? (
                      <button
                        className="ghost-button"
                        disabled={busy !== null}
                        onClick={() =>
                          void runMutation(
                            'mark-complete',
                            markActiveTaskCompleted,
                          )
                        }
                        type="button"
                      >
                        标记完成并进入 overlearning
                      </button>
                    ) : null}
                  </div>
                  {activeTimer.overlearningStartedAt ? (
                    <div className="notice-box">
                      任务已完成，当前剩余时间计入 overlearning。
                    </div>
                  ) : null}
                </>
              ) : (
                <div className="empty-state large">
                  当前没有 active timer。先在今天的待办里选一个任务开始专注。
                </div>
              )}
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Interruption</p>
                  <h3>中断处理</h3>
                </div>
              </div>
              {activeTimer?.phaseType === 'focus' ? (
                <form
                  className="task-form"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void runMutation('record-interruption', async () => {
                      await recordInterruption(interruption);
                      setInterruption({
                        source: 'external',
                        note: '',
                        resolution: 'postpone',
                      });
                    });
                  }}
                >
                  <div className="inline-fields">
                    <label>
                      Source
                      <select
                        value={interruption.source}
                        onChange={(event) =>
                          setInterruption((current) => ({
                            ...current,
                            source: event.target.value as
                              | 'internal'
                              | 'external',
                          }))
                        }
                      >
                        <option value="external">external</option>
                        <option value="internal">internal</option>
                      </select>
                    </label>
                    <label>
                      Resolution
                      <select
                        value={interruption.resolution}
                        onChange={(event) =>
                          setInterruption((current) => ({
                            ...current,
                            resolution: event.target.value as
                              | 'postpone'
                              | 'pause'
                              | 'abort',
                          }))
                        }
                      >
                        <option value="postpone">postpone</option>
                        <option value="pause">pause</option>
                        <option value="abort">abort</option>
                      </select>
                    </label>
                  </div>
                  <textarea
                    placeholder="记录打断内容"
                    value={interruption.note}
                    onChange={(event) =>
                      setInterruption((current) => ({
                        ...current,
                        note: event.target.value,
                      }))
                    }
                  />
                  <button className="primary-button" disabled={busy !== null} type="submit">
                    记录中断
                  </button>
                </form>
              ) : (
                <div className="empty-state">
                  只有 focus session 允许记录 interruption。
                </div>
              )}

              <div className="mini-list">
                <h4>最近中断</h4>
                {(data?.recentInterruptions ?? []).map((item) => (
                  <div className="mini-item" key={item.id}>
                    <div>
                      <strong>{item.taskTitle ?? '未关联任务'}</strong>
                      <p>
                        {item.source} · {item.resolution} · {formatDateTime(item.createdAt)}
                      </p>
                    </div>
                    <span>{item.note || '无备注'}</span>
                  </div>
                ))}
              </div>
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Quick Start</p>
                  <h3>今日可启动任务</h3>
                </div>
              </div>
              <div className="stack">
                {todoTasks.length ? (
                  todoTasks.map((task) => (
                    <div className="task-row compact" key={task.id}>
                      <div>
                        <strong>{task.title}</strong>
                        <p>
                          P{task.priority} · {task.actualPomodoros}/
                          {task.estimatedPomodoros}
                        </p>
                      </div>
                      <button
                        className="primary-button"
                        disabled={busy !== null || activeTimer !== null}
                        onClick={() =>
                          void runMutation('start-focus', () =>
                            startFocusSession(task.id),
                          )
                        }
                        type="button"
                      >
                        开始
                      </button>
                    </div>
                  ))
                ) : (
                  <div className="empty-state">没有可启动任务。</div>
                )}
              </div>
            </article>
          </section>
        ) : null}

        {panel === 'Records' ? (
          <section className="records-layout">
            <article className="card stretch">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Sessions</p>
                  <h3>最近 12 条 session</h3>
                </div>
              </div>
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>Phase</th>
                      <th>Task</th>
                      <th>Status</th>
                      <th>Focus</th>
                      <th>Overlearning</th>
                      <th>Paused</th>
                      <th>Started</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(data?.recentSessions ?? []).map((session) => (
                      <tr key={session.id}>
                        <td>{session.phaseType}</td>
                        <td>{session.taskTitle ?? '—'}</td>
                        <td>{STATUS_LABELS[session.status] ?? session.status}</td>
                        <td>{formatMinutes(session.focusSeconds)}</td>
                        <td>{formatMinutes(session.overlearningSeconds)}</td>
                        <td>{formatMinutes(session.pausedSeconds)}</td>
                        <td>{formatDateTime(session.startedAt)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Interruptions</p>
                  <h3>最近 12 条 interruption</h3>
                </div>
              </div>
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>Task</th>
                      <th>Source</th>
                      <th>Resolution</th>
                      <th>Note</th>
                      <th>Time</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(data?.recentInterruptions ?? []).map((item) => (
                      <tr key={item.id}>
                        <td>{item.taskTitle ?? '—'}</td>
                        <td>{item.source}</td>
                        <td>{item.resolution}</td>
                        <td>{item.note || '—'}</td>
                        <td>{formatDateTime(item.createdAt)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </article>
          </section>
        ) : null}

        {panel === 'Analytics' ? (
          <section className="analytics-layout">
            <article className="card stretch">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Trend</p>
                  <h3>最近 14 天 focus 分钟</h3>
                </div>
              </div>
              <TrendLine stats={data?.dailyStats ?? []} />
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Completed vs Aborted</p>
                  <h3>最近 7 天</h3>
                </div>
              </div>
              <CompareBars stats={data?.dailyStats ?? []} />
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Day Compare</p>
                  <h3>今日 vs 昨日</h3>
                </div>
              </div>
              <div className="compare-grid">
                <div className="compare-box">
                  <span>今日 Focus</span>
                  <strong>{todayStat?.focusMinutes ?? 0} min</strong>
                </div>
                <div className="compare-box">
                  <span>昨日 Focus</span>
                  <strong>{yesterdayStat?.focusMinutes ?? 0} min</strong>
                </div>
                <div className="compare-box">
                  <span>今日 Completed</span>
                  <strong>{todayStat?.completedPomodoros ?? 0}</strong>
                </div>
                <div className="compare-box">
                  <span>昨日 Completed</span>
                  <strong>{yesterdayStat?.completedPomodoros ?? 0}</strong>
                </div>
              </div>
            </article>

            <article className="card">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Week Compare</p>
                  <h3>本周 vs 上周</h3>
                </div>
              </div>
              <div className="compare-grid">
                <div className="compare-box">
                  <span>本周 Focus</span>
                  <strong>{thisWeek.focus} min</strong>
                </div>
                <div className="compare-box">
                  <span>上周 Focus</span>
                  <strong>{lastWeek.focus} min</strong>
                </div>
                <div className="compare-box">
                  <span>本周 Completed</span>
                  <strong>{thisWeek.completed}</strong>
                </div>
                <div className="compare-box">
                  <span>上周 Completed</span>
                  <strong>{lastWeek.completed}</strong>
                </div>
              </div>
            </article>

            <article className="card stretch">
              <div className="card-header">
                <div>
                  <p className="eyebrow">Estimate Audit</p>
                  <h3>今日任务预估 vs 实际</h3>
                </div>
              </div>
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>Task</th>
                      <th>Estimate</th>
                      <th>Actual</th>
                      <th>Delta</th>
                    </tr>
                  </thead>
                  <tbody>
                    {todayTasks.map((task) => (
                      <tr key={task.id}>
                        <td>{task.title}</td>
                        <td>{task.estimatedPomodoros}</td>
                        <td>{task.actualPomodoros}</td>
                        <td>{task.actualPomodoros - task.estimatedPomodoros}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </article>
          </section>
        ) : null}
      </main>
    </div>
  );
}
