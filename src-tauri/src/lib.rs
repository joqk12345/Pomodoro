use std::collections::HashMap;
use std::fs;
use std::io::{Error as IoError, ErrorKind};

use chrono::{DateTime, Days, Duration, Local, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

const ACTIVE_TIMER_KEY: &str = "active_timer";
const CYCLE_FOCUS_COUNT_KEY: &str = "cycle_focus_count";
const FOCUS_SECONDS: i64 = 25 * 60;
const SHORT_BREAK_SECONDS: i64 = 5 * 60;
const LONG_BREAK_SECONDS: i64 = 15 * 60;

type AppResult<T> = Result<T, String>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskView {
    id: String,
    title: String,
    priority: i64,
    estimated_pomodoros: i64,
    actual_pomodoros: i64,
    status: String,
    today_order: i64,
    notes: String,
    scheduled_date: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveTimerView {
    session_id: String,
    task_id: Option<String>,
    task_title: Option<String>,
    phase_type: String,
    status: String,
    started_at: String,
    ends_at: String,
    remaining_seconds: i64,
    planned_seconds: i64,
    pomodoro_index: i64,
    overlearning_started_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionView {
    id: String,
    phase_type: String,
    status: String,
    task_id: Option<String>,
    task_title: Option<String>,
    started_at: String,
    ended_at: Option<String>,
    focus_seconds: i64,
    overlearning_seconds: i64,
    paused_seconds: i64,
    pomodoro_index: i64,
    day_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InterruptionView {
    id: String,
    session_id: String,
    task_title: Option<String>,
    source: String,
    note: String,
    resolution: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewStats {
    completed_pomodoros: i64,
    aborted_pomodoros: i64,
    focus_minutes: i64,
    overlearning_minutes: i64,
    completed_tasks: i64,
    interruptions_today: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DailyStat {
    day_key: String,
    focus_minutes: i64,
    completed_pomodoros: i64,
    aborted_pomodoros: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    tasks: Vec<TaskView>,
    overdue_tasks: Vec<TaskView>,
    active_timer: Option<ActiveTimerView>,
    recent_sessions: Vec<SessionView>,
    recent_interruptions: Vec<InterruptionView>,
    overview: OverviewStats,
    daily_stats: Vec<DailyStat>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskInput {
    title: String,
    priority: i64,
    estimated_pomodoros: i64,
    notes: String,
    scheduled_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterruptionInput {
    source: String,
    note: String,
    resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredActiveTimer {
    session_id: String,
    task_id: Option<String>,
    task_title: Option<String>,
    phase_type: String,
    status: String,
    started_at: String,
    ends_at: String,
    remaining_seconds: i64,
    planned_seconds: i64,
    pomodoro_index: i64,
    overlearning_started_at: Option<String>,
    paused_at: Option<String>,
    last_resumed_at: Option<String>,
    focus_elapsed_seconds: i64,
    overlearning_elapsed_seconds: i64,
    paused_seconds: i64,
}

impl StoredActiveTimer {
    fn to_view(&self) -> ActiveTimerView {
        ActiveTimerView {
            session_id: self.session_id.clone(),
            task_id: self.task_id.clone(),
            task_title: self.task_title.clone(),
            phase_type: self.phase_type.clone(),
            status: self.status.clone(),
            started_at: self.started_at.clone(),
            ends_at: self.ends_at.clone(),
            remaining_seconds: self.remaining_seconds,
            planned_seconds: self.planned_seconds,
            pomodoro_index: self.pomodoro_index,
            overlearning_started_at: self.overlearning_started_at.clone(),
        }
    }
}

fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

fn iso_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn parse_iso(value: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|error| error.to_string())
}

fn today_key() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn local_day_key(value: DateTime<Utc>) -> String {
    value.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

fn validate_day_key(value: &str) -> AppResult<()> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| format!("invalid date: {value}"))
}

fn ensure_in_choices(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("invalid {field}: {value}"))
    }
}

fn validate_task_input(input: &TaskInput) -> AppResult<()> {
    if input.title.trim().is_empty() {
        return Err("task title cannot be empty".into());
    }

    if !(1..=5).contains(&input.priority) {
        return Err("priority must be between 1 and 5".into());
    }

    if !(1..=12).contains(&input.estimated_pomodoros) {
        return Err("estimated pomodoros must be between 1 and 12".into());
    }

    if let Some(date) = &input.scheduled_date {
        validate_day_key(date)?;
    }

    Ok(())
}

fn validate_interruption_input(input: &InterruptionInput) -> AppResult<()> {
    ensure_in_choices(&input.source, &["internal", "external"], "source")?;
    ensure_in_choices(
        &input.resolution,
        &["postpone", "pause", "abort"],
        "resolution",
    )?;
    Ok(())
}

fn phase_seconds(phase_type: &str) -> AppResult<i64> {
    match phase_type {
        "focus" => Ok(FOCUS_SECONDS),
        "short_break" => Ok(SHORT_BREAK_SECONDS),
        "long_break" => Ok(LONG_BREAK_SECONDS),
        value => Err(format!("invalid phase type: {value}")),
    }
}

fn db_path(app: &AppHandle) -> AppResult<std::path::PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("pomodoro.sqlite3"))
}

fn open_connection(app: &AppHandle) -> AppResult<Connection> {
    let path = db_path(app)?;
    let conn = Connection::open(path).map_err(|error| error.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| error.to_string())?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tasks (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          priority INTEGER NOT NULL,
          estimated_pomodoros INTEGER NOT NULL,
          status TEXT NOT NULL CHECK(status IN ('todo', 'done')),
          today_order INTEGER NOT NULL,
          notes TEXT NOT NULL DEFAULT '',
          scheduled_date TEXT NOT NULL,
          completed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY,
          phase_type TEXT NOT NULL CHECK(phase_type IN ('focus', 'short_break', 'long_break')),
          status TEXT NOT NULL CHECK(status IN ('active', 'paused', 'completed', 'aborted')),
          task_id TEXT,
          task_title TEXT,
          started_at TEXT NOT NULL,
          ended_at TEXT,
          focus_seconds INTEGER NOT NULL DEFAULT 0,
          overlearning_seconds INTEGER NOT NULL DEFAULT 0,
          paused_seconds INTEGER NOT NULL DEFAULT 0,
          pomodoro_index INTEGER NOT NULL,
          day_key TEXT NOT NULL,
          FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS interruptions (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          task_title TEXT,
          source TEXT NOT NULL CHECK(source IN ('internal', 'external')),
          note TEXT NOT NULL,
          resolution TEXT NOT NULL CHECK(resolution IN ('postpone', 'pause', 'abort')),
          created_at TEXT NOT NULL,
          FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS app_state (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_schedule ON tasks(scheduled_date, status, today_order);
        CREATE INDEX IF NOT EXISTS idx_sessions_day_key ON sessions(day_key, phase_type, status);
        CREATE INDEX IF NOT EXISTS idx_interruptions_created_at ON interruptions(created_at DESC);
        ",
    )
    .map_err(|error| error.to_string())
}

fn get_state_value(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    conn.query_row("SELECT value FROM app_state WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
    .map_err(|error| error.to_string())
}

fn set_state_value(conn: &Connection, key: &str, value: Option<&str>) -> AppResult<()> {
    match value {
        Some(value) => conn
            .execute(
                "
                INSERT INTO app_state (key, value) VALUES (?1, ?2)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                ",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|error| error.to_string()),
        None => conn
            .execute("DELETE FROM app_state WHERE key = ?1", [key])
            .map(|_| ())
            .map_err(|error| error.to_string()),
    }
}

fn get_active_timer(conn: &Connection) -> AppResult<Option<StoredActiveTimer>> {
    match get_state_value(conn, ACTIVE_TIMER_KEY)? {
        Some(value) => serde_json::from_str(&value).map(Some).map_err(|error| error.to_string()),
        None => Ok(None),
    }
}

fn save_active_timer(conn: &Connection, timer: Option<&StoredActiveTimer>) -> AppResult<()> {
    match timer {
        Some(timer) => {
            let json = serde_json::to_string(timer).map_err(|error| error.to_string())?;
            set_state_value(conn, ACTIVE_TIMER_KEY, Some(&json))
        }
        None => set_state_value(conn, ACTIVE_TIMER_KEY, None),
    }
}

fn get_cycle_focus_count(conn: &Connection) -> AppResult<i64> {
    Ok(get_state_value(conn, CYCLE_FOCUS_COUNT_KEY)?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0))
}

fn set_cycle_focus_count(conn: &Connection, value: i64) -> AppResult<()> {
    set_state_value(conn, CYCLE_FOCUS_COUNT_KEY, Some(&value.to_string()))
}

fn new_timer(
    phase_type: &str,
    task_id: Option<String>,
    task_title: Option<String>,
    start_at: DateTime<Utc>,
    pomodoro_index: i64,
) -> AppResult<StoredActiveTimer> {
    let planned_seconds = phase_seconds(phase_type)?;
    let session_id = Uuid::new_v4().to_string();
    Ok(StoredActiveTimer {
        session_id,
        task_id,
        task_title,
        phase_type: phase_type.to_string(),
        status: "running".to_string(),
        started_at: iso_string(start_at),
        ends_at: iso_string(start_at + Duration::seconds(planned_seconds)),
        remaining_seconds: planned_seconds,
        planned_seconds,
        pomodoro_index,
        overlearning_started_at: None,
        paused_at: None,
        last_resumed_at: Some(iso_string(start_at)),
        focus_elapsed_seconds: 0,
        overlearning_elapsed_seconds: 0,
        paused_seconds: 0,
    })
}

fn insert_session(conn: &Connection, timer: &StoredActiveTimer) -> AppResult<()> {
    let started_at = parse_iso(&timer.started_at)?;
    conn.execute(
        "
        INSERT INTO sessions (
          id, phase_type, status, task_id, task_title, started_at, ended_at,
          focus_seconds, overlearning_seconds, paused_seconds, pomodoro_index, day_key
        ) VALUES (?1, ?2, 'active', ?3, ?4, ?5, NULL, 0, 0, 0, ?6, ?7)
        ",
        params![
            timer.session_id,
            timer.phase_type,
            timer.task_id,
            timer.task_title,
            timer.started_at,
            timer.pomodoro_index,
            local_day_key(started_at),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn accumulate_focus_segment(timer: &mut StoredActiveTimer, until: DateTime<Utc>) -> AppResult<()> {
    if timer.phase_type != "focus" {
        return Ok(());
    }

    let Some(last_resumed_at) = &timer.last_resumed_at else {
        return Ok(());
    };

    let start = parse_iso(last_resumed_at)?;
    if until <= start {
        return Ok(());
    }

    let total = (until - start).num_seconds();

    if let Some(overlearning_started_at) = &timer.overlearning_started_at {
        let overlearning_start = parse_iso(overlearning_started_at)?;
        if overlearning_start <= start {
            timer.overlearning_elapsed_seconds += total;
        } else if overlearning_start >= until {
            timer.focus_elapsed_seconds += total;
        } else {
            timer.focus_elapsed_seconds += (overlearning_start - start).num_seconds();
            timer.overlearning_elapsed_seconds += (until - overlearning_start).num_seconds();
        }
    } else {
        timer.focus_elapsed_seconds += total;
    }

    Ok(())
}

fn pause_loaded_timer(
    conn: &Connection,
    timer: &mut StoredActiveTimer,
    at: DateTime<Utc>,
) -> AppResult<()> {
    if timer.status != "running" {
        return Err("active timer is not running".into());
    }

    accumulate_focus_segment(timer, at)?;

    let ends_at = parse_iso(&timer.ends_at)?;
    timer.remaining_seconds = (ends_at - at).num_seconds().max(0);
    timer.status = "paused".into();
    timer.paused_at = Some(iso_string(at));
    timer.last_resumed_at = None;

    conn.execute(
        "UPDATE sessions SET status = 'paused' WHERE id = ?1",
        [&timer.session_id],
    )
    .map_err(|error| error.to_string())?;
    save_active_timer(conn, Some(timer))
}

fn resume_loaded_timer(
    conn: &Connection,
    timer: &mut StoredActiveTimer,
    at: DateTime<Utc>,
) -> AppResult<()> {
    if timer.status != "paused" {
        return Err("active timer is not paused".into());
    }

    if let Some(paused_at) = &timer.paused_at {
        let paused = parse_iso(paused_at)?;
        if at > paused {
            timer.paused_seconds += (at - paused).num_seconds();
        }
    }

    timer.status = "running".into();
    timer.paused_at = None;
    timer.last_resumed_at = Some(iso_string(at));
    timer.ends_at = iso_string(at + Duration::seconds(timer.remaining_seconds));

    conn.execute(
        "UPDATE sessions SET status = 'active' WHERE id = ?1",
        [&timer.session_id],
    )
    .map_err(|error| error.to_string())?;
    save_active_timer(conn, Some(timer))
}

fn notify_phase(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

fn finalize_loaded_timer(
    conn: &Connection,
    app: &AppHandle,
    mut timer: StoredActiveTimer,
    final_status: &str,
    finish_at: DateTime<Utc>,
) -> AppResult<()> {
    if timer.status == "running" {
        accumulate_focus_segment(&mut timer, finish_at)?;
    }

    conn.execute(
        "
        UPDATE sessions
        SET status = ?2,
            ended_at = ?3,
            focus_seconds = ?4,
            overlearning_seconds = ?5,
            paused_seconds = ?6
        WHERE id = ?1
        ",
        params![
            timer.session_id,
            final_status,
            iso_string(finish_at),
            timer.focus_elapsed_seconds.max(0),
            timer.overlearning_elapsed_seconds.max(0),
            timer.paused_seconds.max(0),
        ],
    )
    .map_err(|error| error.to_string())?;

    match (timer.phase_type.as_str(), final_status) {
        ("focus", "completed") => {
            let next_count = get_cycle_focus_count(conn)? + 1;
            set_cycle_focus_count(conn, next_count)?;
            let next_phase = if next_count >= 4 {
                "long_break"
            } else {
                "short_break"
            };
            let break_timer = new_timer(next_phase, None, None, finish_at, next_count)?;
            insert_session(conn, &break_timer)?;
            save_active_timer(conn, Some(&break_timer))?;
            notify_phase(app, "Focus finished", "Time for a break.");
        }
        ("short_break", "completed") => {
            save_active_timer(conn, None)?;
            notify_phase(app, "Break finished", "Pick the next task when ready.");
        }
        ("long_break", "completed") => {
            set_cycle_focus_count(conn, 0)?;
            save_active_timer(conn, None)?;
            notify_phase(app, "Long break finished", "Cycle reset.");
        }
        ("focus", "aborted") => {
            save_active_timer(conn, None)?;
        }
        _ => {
            save_active_timer(conn, None)?;
        }
    }

    Ok(())
}

fn sync_active_timer(conn: &Connection, app: &AppHandle) -> AppResult<()> {
    loop {
        let Some(timer) = get_active_timer(conn)? else {
            return Ok(());
        };

        if timer.status != "running" {
            return Ok(());
        }

        let ends_at = parse_iso(&timer.ends_at)?;
        let now = now_utc();
        if now < ends_at {
            return Ok(());
        }

        finalize_loaded_timer(conn, app, timer, "completed", ends_at)?;
    }
}

fn active_task_id(conn: &Connection) -> AppResult<Option<String>> {
    Ok(get_active_timer(conn)?.and_then(|timer| timer.task_id))
}

fn query_task_views(conn: &Connection, sql: &str, arg: &str) -> AppResult<Vec<TaskView>> {
    let mut statement = conn.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([arg], |row| {
            Ok(TaskView {
                id: row.get(0)?,
                title: row.get(1)?,
                priority: row.get(2)?,
                estimated_pomodoros: row.get(3)?,
                actual_pomodoros: row.get(4)?,
                status: row.get(5)?,
                today_order: row.get(6)?,
                notes: row.get(7)?,
                scheduled_date: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn query_recent_sessions(conn: &Connection) -> AppResult<Vec<SessionView>> {
    let mut statement = conn
        .prepare(
            "
            SELECT id, phase_type, status, task_id, task_title, started_at, ended_at,
                   focus_seconds, overlearning_seconds, paused_seconds, pomodoro_index, day_key
            FROM sessions
            ORDER BY started_at DESC
            LIMIT 12
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(SessionView {
                id: row.get(0)?,
                phase_type: row.get(1)?,
                status: row.get(2)?,
                task_id: row.get(3)?,
                task_title: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                focus_seconds: row.get(7)?,
                overlearning_seconds: row.get(8)?,
                paused_seconds: row.get(9)?,
                pomodoro_index: row.get(10)?,
                day_key: row.get(11)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn query_all_sessions(conn: &Connection) -> AppResult<Vec<SessionView>> {
    let mut statement = conn
        .prepare(
            "
            SELECT id, phase_type, status, task_id, task_title, started_at, ended_at,
                   focus_seconds, overlearning_seconds, paused_seconds, pomodoro_index, day_key
            FROM sessions
            ORDER BY started_at DESC
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(SessionView {
                id: row.get(0)?,
                phase_type: row.get(1)?,
                status: row.get(2)?,
                task_id: row.get(3)?,
                task_title: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                focus_seconds: row.get(7)?,
                overlearning_seconds: row.get(8)?,
                paused_seconds: row.get(9)?,
                pomodoro_index: row.get(10)?,
                day_key: row.get(11)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn query_recent_interruptions(conn: &Connection) -> AppResult<Vec<InterruptionView>> {
    let mut statement = conn
        .prepare(
            "
            SELECT id, session_id, task_title, source, note, resolution, created_at
            FROM interruptions
            ORDER BY created_at DESC
            LIMIT 12
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(InterruptionView {
                id: row.get(0)?,
                session_id: row.get(1)?,
                task_title: row.get(2)?,
                source: row.get(3)?,
                note: row.get(4)?,
                resolution: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn query_all_interruptions(conn: &Connection) -> AppResult<Vec<InterruptionView>> {
    let mut statement = conn
        .prepare(
            "
            SELECT id, session_id, task_title, source, note, resolution, created_at
            FROM interruptions
            ORDER BY created_at DESC
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(InterruptionView {
                id: row.get(0)?,
                session_id: row.get(1)?,
                task_title: row.get(2)?,
                source: row.get(3)?,
                note: row.get(4)?,
                resolution: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn query_overview(conn: &Connection) -> AppResult<OverviewStats> {
    let today = today_key();

    let (completed_pomodoros, aborted_pomodoros, focus_seconds, overlearning_seconds): (
        i64,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "
            SELECT
              COALESCE(SUM(CASE WHEN phase_type = 'focus' AND status = 'completed' THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN phase_type = 'focus' AND status = 'aborted' THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN phase_type = 'focus' THEN focus_seconds ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN phase_type = 'focus' THEN overlearning_seconds ELSE 0 END), 0)
            FROM sessions
            WHERE day_key = ?1
            ",
            [&today],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| error.to_string())?;

    let completed_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'done' AND completed_at IS NOT NULL AND substr(completed_at, 1, 10) = ?1",
            [&today],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;

    let interruptions_today: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interruptions WHERE substr(created_at, 1, 10) = ?1",
            [&today],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;

    Ok(OverviewStats {
        completed_pomodoros,
        aborted_pomodoros,
        focus_minutes: focus_seconds / 60,
        overlearning_minutes: overlearning_seconds / 60,
        completed_tasks,
        interruptions_today,
    })
}

fn query_daily_stats(conn: &Connection) -> AppResult<Vec<DailyStat>> {
    let today = Local::now().date_naive();
    let start = today
        .checked_sub_days(Days::new(13))
        .ok_or_else(|| "failed to calculate start day".to_string())?;
    let mut bucket: HashMap<String, DailyStat> = HashMap::new();

    let mut statement = conn
        .prepare(
            "
            SELECT day_key,
                   COALESCE(SUM(CASE WHEN phase_type = 'focus' THEN focus_seconds ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN phase_type = 'focus' AND status = 'completed' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN phase_type = 'focus' AND status = 'aborted' THEN 1 ELSE 0 END), 0)
            FROM sessions
            WHERE day_key >= ?1
            GROUP BY day_key
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([start.format("%Y-%m-%d").to_string()], |row| {
            Ok(DailyStat {
                day_key: row.get(0)?,
                focus_minutes: row.get::<_, i64>(1)? / 60,
                completed_pomodoros: row.get(2)?,
                aborted_pomodoros: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?;

    for row in rows {
        let stat = row.map_err(|error| error.to_string())?;
        bucket.insert(stat.day_key.clone(), stat);
    }

    let mut result = Vec::with_capacity(14);
    for offset in 0..14 {
        let day = start
            .checked_add_days(Days::new(offset))
            .ok_or_else(|| "failed to calculate daily stat".to_string())?;
        let key = day.format("%Y-%m-%d").to_string();
        result.push(bucket.remove(&key).unwrap_or(DailyStat {
            day_key: key,
            focus_minutes: 0,
            completed_pomodoros: 0,
            aborted_pomodoros: 0,
        }));
    }

    Ok(result)
}

fn get_snapshot(conn: &Connection) -> AppResult<AppSnapshot> {
    let today = today_key();
    let tasks = query_task_views(
        conn,
        "
        SELECT
          t.id,
          t.title,
          t.priority,
          t.estimated_pomodoros,
          COALESCE((
            SELECT COUNT(*)
            FROM sessions s
            WHERE s.task_id = t.id
              AND s.phase_type = 'focus'
              AND s.status = 'completed'
          ), 0) AS actual_pomodoros,
          t.status,
          t.today_order,
          t.notes,
          t.scheduled_date
        FROM tasks t
        WHERE t.scheduled_date = ?1
        ORDER BY CASE WHEN t.status = 'todo' THEN 0 ELSE 1 END, t.today_order ASC, t.title ASC
        ",
        &today,
    )?;

    let overdue_tasks = query_task_views(
        conn,
        "
        SELECT
          t.id,
          t.title,
          t.priority,
          t.estimated_pomodoros,
          COALESCE((
            SELECT COUNT(*)
            FROM sessions s
            WHERE s.task_id = t.id
              AND s.phase_type = 'focus'
              AND s.status = 'completed'
          ), 0) AS actual_pomodoros,
          t.status,
          t.today_order,
          t.notes,
          t.scheduled_date
        FROM tasks t
        WHERE t.status = 'todo' AND t.scheduled_date < ?1
        ORDER BY t.scheduled_date ASC, t.today_order ASC
        ",
        &today,
    )?;

    Ok(AppSnapshot {
        tasks,
        overdue_tasks,
        active_timer: get_active_timer(conn)?.map(|timer| timer.to_view()),
        recent_sessions: query_recent_sessions(conn)?,
        recent_interruptions: query_recent_interruptions(conn)?,
        overview: query_overview(conn)?,
        daily_stats: query_daily_stats(conn)?,
    })
}

fn fetch_task_for_update(
    conn: &Connection,
    task_id: &str,
) -> AppResult<(String, i64, i64, String, String, Option<String>)> {
    conn.query_row(
        "
        SELECT title, priority, estimated_pomodoros, notes, scheduled_date, completed_at
        FROM tasks WHERE id = ?1
        ",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        },
    )
    .optional()
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "task not found".into())
}

#[tauri::command]
fn get_app_snapshot(app: AppHandle) -> AppResult<AppSnapshot> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    get_snapshot(&conn)
}

#[tauri::command]
fn export_data(app: AppHandle) -> AppResult<String> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    let tasks = query_task_views(
        &conn,
        "
        SELECT
          t.id,
          t.title,
          t.priority,
          t.estimated_pomodoros,
          COALESCE((
            SELECT COUNT(*)
            FROM sessions s
            WHERE s.task_id = t.id
              AND s.phase_type = 'focus'
              AND s.status = 'completed'
          ), 0),
          t.status,
          t.today_order,
          t.notes,
          t.scheduled_date
        FROM tasks t
        WHERE t.scheduled_date >= ?1 OR t.scheduled_date < ?1
        ORDER BY t.scheduled_date ASC, t.today_order ASC
        ",
        "0000-01-01",
    )?;
    let sessions = query_all_sessions(&conn)?;
    let interruptions = query_all_interruptions(&conn)?;
    let active_timer = get_active_timer(&conn)?;
    let cycle_focus_count = get_cycle_focus_count(&conn)?;

    serde_json::to_string_pretty(&json!({
        "tasks": tasks,
        "sessions": sessions,
        "interruptions": interruptions,
        "appState": {
            "activeTimer": active_timer,
            "cycleFocusCount": cycle_focus_count
        }
    }))
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_task(app: AppHandle, input: TaskInput) -> AppResult<()> {
    validate_task_input(&input)?;
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    let scheduled_date = input.scheduled_date.unwrap_or_else(today_key);
    let next_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(today_order), -1) + 1 FROM tasks WHERE scheduled_date = ?1",
            [&scheduled_date],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;

    conn.execute(
        "
        INSERT INTO tasks (
          id, title, priority, estimated_pomodoros, status, today_order, notes, scheduled_date, completed_at
        ) VALUES (?1, ?2, ?3, ?4, 'todo', ?5, ?6, ?7, NULL)
        ",
        params![
            Uuid::new_v4().to_string(),
            input.title.trim(),
            input.priority,
            input.estimated_pomodoros,
            next_order,
            input.notes.trim(),
            scheduled_date,
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn update_task(app: AppHandle, task_id: String, input: TaskInput) -> AppResult<()> {
    validate_task_input(&input)?;
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    let (_, _, _, _, current_date, completed_at) = fetch_task_for_update(&conn, &task_id)?;
    let scheduled_date = input
        .scheduled_date
        .clone()
        .unwrap_or_else(|| current_date.clone());

    if active_task_id(&conn)? == Some(task_id.clone()) && scheduled_date != current_date {
        return Err("active task cannot be rescheduled".into());
    }

    let next_order: i64 = if scheduled_date == current_date {
        conn.query_row(
            "SELECT today_order FROM tasks WHERE id = ?1",
            [&task_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    } else {
        conn.query_row(
            "SELECT COALESCE(MAX(today_order), -1) + 1 FROM tasks WHERE scheduled_date = ?1",
            [&scheduled_date],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    };

    conn.execute(
        "
        UPDATE tasks
        SET title = ?2,
            priority = ?3,
            estimated_pomodoros = ?4,
            notes = ?5,
            scheduled_date = ?6,
            today_order = ?7,
            completed_at = ?8
        WHERE id = ?1
        ",
        params![
            task_id,
            input.title.trim(),
            input.priority,
            input.estimated_pomodoros,
            input.notes.trim(),
            scheduled_date,
            next_order,
            completed_at,
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn move_task(app: AppHandle, task_id: String, direction: String) -> AppResult<()> {
    ensure_in_choices(&direction, &["up", "down"], "direction")?;
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    let today = today_key();
    let (current_order, scheduled_date, status): (i64, String, String) = conn
        .query_row(
            "SELECT today_order, scheduled_date, status FROM tasks WHERE id = ?1",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task not found".to_string())?;

    if scheduled_date != today || status != "todo" {
        return Err("only today's todo tasks can be moved".into());
    }

    let neighbor = if direction == "up" {
        conn.query_row(
            "
            SELECT id, today_order FROM tasks
            WHERE scheduled_date = ?1 AND status = 'todo' AND today_order < ?2
            ORDER BY today_order DESC
            LIMIT 1
            ",
            params![today, current_order],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
    } else {
        conn.query_row(
            "
            SELECT id, today_order FROM tasks
            WHERE scheduled_date = ?1 AND status = 'todo' AND today_order > ?2
            ORDER BY today_order ASC
            LIMIT 1
            ",
            params![today, current_order],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
    }
    .optional()
    .map_err(|error| error.to_string())?;

    if let Some((neighbor_id, neighbor_order)) = neighbor {
        conn.execute(
            "UPDATE tasks SET today_order = ?2 WHERE id = ?1",
            params![task_id, neighbor_order],
        )
        .map_err(|error| error.to_string())?;
        conn.execute(
            "UPDATE tasks SET today_order = ?2 WHERE id = ?1",
            params![neighbor_id, current_order],
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn delete_task(app: AppHandle, task_id: String) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    if active_task_id(&conn)? == Some(task_id.clone()) {
        return Err("active task cannot be deleted".into());
    }

    conn.execute("DELETE FROM tasks WHERE id = ?1", [&task_id])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_task_completion(app: AppHandle, task_id: String, completed: bool) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    if active_task_id(&conn)? == Some(task_id.clone()) {
        return Err("use mark_active_task_completed for the active task".into());
    }

    conn.execute(
        "
        UPDATE tasks
        SET status = ?2, completed_at = ?3
        WHERE id = ?1
        ",
        params![
            task_id,
            if completed { "done" } else { "todo" },
            if completed {
                Some(iso_string(now_utc()))
            } else {
                None
            }
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_focus_session(app: AppHandle, task_id: String) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;

    if get_active_timer(&conn)?.is_some() {
        return Err("an active timer already exists".into());
    }

    let (title, status): (String, String) = conn
        .query_row(
            "SELECT title, status FROM tasks WHERE id = ?1",
            [&task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task not found".to_string())?;

    if status != "todo" {
        return Err("only todo tasks can start focus".into());
    }

    let pomodoro_index = get_cycle_focus_count(&conn)? + 1;
    let timer = new_timer(
        "focus",
        Some(task_id),
        Some(title),
        now_utc(),
        pomodoro_index,
    )?;
    insert_session(&conn, &timer)?;
    save_active_timer(&conn, Some(&timer))
}

#[tauri::command]
fn mark_active_task_completed(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    let mut timer = get_active_timer(&conn)?.ok_or_else(|| "no active timer".to_string())?;

    if timer.phase_type != "focus" || timer.status != "running" {
        return Err("only a running focus session can enter overlearning".into());
    }

    let task_id = timer
        .task_id
        .clone()
        .ok_or_else(|| "active focus session has no task".to_string())?;

    if timer.overlearning_started_at.is_some() {
        return Err("task is already in overlearning".into());
    }

    let at = now_utc();
    conn.execute(
        "UPDATE tasks SET status = 'done', completed_at = ?2 WHERE id = ?1",
        params![task_id, iso_string(at)],
    )
    .map_err(|error| error.to_string())?;
    timer.overlearning_started_at = Some(iso_string(at));
    save_active_timer(&conn, Some(&timer))
}

#[tauri::command]
fn pause_active_timer(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    let mut timer = get_active_timer(&conn)?.ok_or_else(|| "no active timer".to_string())?;
    pause_loaded_timer(&conn, &mut timer, now_utc())
}

#[tauri::command]
fn resume_active_timer(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    let mut timer = get_active_timer(&conn)?.ok_or_else(|| "no active timer".to_string())?;
    resume_loaded_timer(&conn, &mut timer, now_utc())
}

#[tauri::command]
fn abort_active_timer(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    let timer = get_active_timer(&conn)?.ok_or_else(|| "no active timer".to_string())?;

    if timer.phase_type != "focus" {
        return Err("only focus sessions can be aborted".into());
    }

    finalize_loaded_timer(&conn, &app, timer, "aborted", now_utc())
}

#[tauri::command]
fn complete_active_timer(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    let timer = get_active_timer(&conn)?.ok_or_else(|| "no active timer".to_string())?;
    finalize_loaded_timer(&conn, &app, timer, "completed", now_utc())
}

#[tauri::command]
fn record_interruption(app: AppHandle, input: InterruptionInput) -> AppResult<()> {
    validate_interruption_input(&input)?;
    let conn = open_connection(&app)?;
    sync_active_timer(&conn, &app)?;
    let mut timer = get_active_timer(&conn)?.ok_or_else(|| "no active focus session".to_string())?;

    if timer.phase_type != "focus" {
        return Err("interruptions can only be recorded on focus sessions".into());
    }

    let created_at = now_utc();
    conn.execute(
        "
        INSERT INTO interruptions (id, session_id, task_title, source, note, resolution, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            Uuid::new_v4().to_string(),
            timer.session_id,
            timer.task_title,
            input.source,
            input.note.trim(),
            input.resolution,
            iso_string(created_at),
        ],
    )
    .map_err(|error| error.to_string())?;

    match input.resolution.as_str() {
        "pause" => pause_loaded_timer(&conn, &mut timer, created_at),
        "abort" => finalize_loaded_timer(&conn, &app, timer, "aborted", created_at),
        _ => Ok(()),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            open_connection(&app.handle())
                .map(|_| ())
                .map_err(|error| IoError::new(ErrorKind::Other, error).into())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_snapshot,
            export_data,
            create_task,
            update_task,
            move_task,
            delete_task,
            toggle_task_completion,
            start_focus_session,
            mark_active_task_completed,
            pause_active_timer,
            resume_active_timer,
            abort_active_timer,
            complete_active_timer,
            record_interruption
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
