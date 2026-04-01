export type PanelKey = 'Today' | 'Focus' | 'Records' | 'Analytics';

export interface TaskView {
  id: string;
  title: string;
  priority: number;
  estimatedPomodoros: number;
  actualPomodoros: number;
  status: 'todo' | 'done';
  todayOrder: number;
  notes: string;
  scheduledDate: string;
}

export interface ActiveTimerView {
  sessionId: string;
  taskId: string | null;
  taskTitle: string | null;
  phaseType: 'focus' | 'short_break' | 'long_break';
  status: 'running' | 'paused';
  startedAt: string;
  endsAt: string;
  remainingSeconds: number;
  plannedSeconds: number;
  pomodoroIndex: number;
  overlearningStartedAt: string | null;
}

export interface SessionView {
  id: string;
  phaseType: 'focus' | 'short_break' | 'long_break';
  status: 'active' | 'paused' | 'completed' | 'aborted';
  taskId: string | null;
  taskTitle: string | null;
  startedAt: string;
  endedAt: string | null;
  focusSeconds: number;
  overlearningSeconds: number;
  pausedSeconds: number;
  pomodoroIndex: number;
  dayKey: string;
}

export interface InterruptionView {
  id: string;
  sessionId: string;
  taskTitle: string | null;
  source: 'internal' | 'external';
  note: string;
  resolution: 'postpone' | 'pause' | 'abort';
  createdAt: string;
}

export interface OverviewStats {
  completedPomodoros: number;
  abortedPomodoros: number;
  focusMinutes: number;
  overlearningMinutes: number;
  completedTasks: number;
  interruptionsToday: number;
}

export interface DailyStat {
  dayKey: string;
  focusMinutes: number;
  completedPomodoros: number;
  abortedPomodoros: number;
}

export interface AppSnapshot {
  tasks: TaskView[];
  overdueTasks: TaskView[];
  activeTimer: ActiveTimerView | null;
  recentSessions: SessionView[];
  recentInterruptions: InterruptionView[];
  overview: OverviewStats;
  dailyStats: DailyStat[];
}

export interface TaskInput {
  title: string;
  priority: number;
  estimatedPomodoros: number;
  notes: string;
  scheduledDate?: string;
}

export interface RecordInterruptionInput {
  source: 'internal' | 'external';
  note: string;
  resolution: 'postpone' | 'pause' | 'abort';
}
