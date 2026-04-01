import { invoke } from '@tauri-apps/api/core';
import type {
  AppSnapshot,
  RecordInterruptionInput,
  TaskInput,
  TaskView,
} from '../types';

export const getAppSnapshot = () => invoke<AppSnapshot>('get_app_snapshot');

export const exportData = () => invoke<string>('export_data');

export const createTask = (input: TaskInput) =>
  invoke<void>('create_task', { input });

export const updateTask = (taskId: string, input: TaskInput) =>
  invoke<void>('update_task', { taskId, input });

export const moveTask = (taskId: string, direction: 'up' | 'down') =>
  invoke<void>('move_task', { taskId, direction });

export const deleteTask = (taskId: string) =>
  invoke<void>('delete_task', { taskId });

export const toggleTaskCompletion = (taskId: string, completed: boolean) =>
  invoke<void>('toggle_task_completion', { taskId, completed });

export const startFocusSession = (taskId: string) =>
  invoke<void>('start_focus_session', { taskId });

export const markActiveTaskCompleted = () =>
  invoke<void>('mark_active_task_completed');

export const pauseActiveTimer = () => invoke<void>('pause_active_timer');

export const resumeActiveTimer = () => invoke<void>('resume_active_timer');

export const abortActiveTimer = () => invoke<void>('abort_active_timer');

export const completeActiveTimer = () => invoke<void>('complete_active_timer');

export const recordInterruption = (input: RecordInterruptionInput) =>
  invoke<void>('record_interruption', { input });

export const cloneTaskToInput = (task: TaskView): TaskInput => ({
  title: task.title,
  priority: task.priority,
  estimatedPomodoros: task.estimatedPomodoros,
  notes: task.notes,
  scheduledDate: task.scheduledDate,
});
