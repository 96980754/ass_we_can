use std::collections::HashSet;

use chrono::{Duration, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};

pub const MIN_VALID_MS: u64 = 1000;
pub const MAX_REP_MS: u64 = 600_000;
pub const DEFAULT_INTERVAL_MIN: u32 = 45;
pub const DEFAULT_TARGET_SEC: u32 = 5;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct BallPos {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 提醒间隔（分钟），5–240，默认 45
    pub interval_minutes: u32,
    /// 每次训练时长（秒），1–60，默认 5：进度环目标与提醒完成标准
    pub target_seconds: u32,
    /// 有效阈值：严格大于该值才计入
    pub min_valid_ms: u64,
    pub sound_enabled: bool,
    /// 悬浮球不透明度 0.3–1.0
    pub opacity: f64,
    pub always_on_top: bool,
    pub theme: String,
    pub paused: bool,
    pub next_due_ms: Option<i64>,
    pub pending_since_ms: Option<i64>,
    pub ball: BallPos,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval_minutes: DEFAULT_INTERVAL_MIN,
            target_seconds: DEFAULT_TARGET_SEC,
            min_valid_ms: MIN_VALID_MS,
            sound_enabled: true,
            opacity: 0.92,
            always_on_top: true,
            theme: "auto".to_string(),
            paused: false,
            next_due_ms: None,
            pending_since_ms: None,
            ball: BallPos::default(),
        }
    }
}

impl Settings {
    pub fn interval_ms(&self) -> i64 {
        self.interval_minutes as i64 * 60_000
    }

    pub fn target_ms(&self) -> u64 {
        self.target_seconds as u64 * 1000
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub interval_minutes: Option<u32>,
    pub target_seconds: Option<u32>,
    pub min_valid_ms: Option<u64>,
    pub sound_enabled: Option<bool>,
    pub opacity: Option<f64>,
    pub always_on_top: Option<bool>,
    pub theme: Option<String>,
    pub paused: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rep {
    pub id: String,
    pub started_at_ms: i64,
    pub duration_ms: u64,
    pub local_date: String,
    pub tz_offset_min: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub date: String,
    pub count: u32,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayHistory {
    pub date: String,
    pub count: u32,
    pub total_ms: u64,
    pub reps: Vec<Rep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub today: DayStat,
    pub all_count: u64,
    pub all_ms: u64,
    pub streak_days: u32,
    pub last7: Vec<DayStat>,
    pub last_rep_ms: Option<i64>,
    pub data_dir: String,
    pub now_ms: i64,
}

pub fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

/// 托盘等紧凑位置使用：45 秒 / 12 分
pub fn format_short(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs} 秒")
    } else {
        format!("{} 分", secs / 60)
    }
}

/// 本地日期键与 UTC 偏移（分钟）。按写入时的本地时区固化，跨时区也不会错位。
pub fn local_date_of(ts_ms: i64) -> (String, i32) {
    match Local.timestamp_millis_opt(ts_ms).single() {
        Some(dt) => (
            dt.format("%Y-%m-%d").to_string(),
            dt.offset().local_minus_utc() / 60,
        ),
        None => ("1970-01-01".to_string(), 0),
    }
}

pub fn today_key() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn day_stat(reps: &[Rep], date: &str) -> DayStat {
    let mut count = 0u32;
    let mut total_ms = 0u64;
    for rep in reps {
        if rep.local_date == date {
            count += 1;
            total_ms += rep.duration_ms;
        }
    }
    DayStat {
        date: date.to_string(),
        count,
        total_ms,
    }
}

pub fn last_n_days(reps: &[Rep], today: &str, n: usize) -> Vec<DayStat> {
    let Ok(end) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return Vec::new();
    };
    (0..n)
        .rev()
        .map(|i| {
            let date = (end - Duration::days(i as i64))
                .format("%Y-%m-%d")
                .to_string();
            day_stat(reps, &date)
        })
        .collect()
}

/// 连续天数：今天有记录就从今天算，否则从昨天算（今天还没练不算中断）。
pub fn streak_days(reps: &[Rep], today: &str) -> u32 {
    let Ok(today_date) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return 0;
    };
    let dates: HashSet<&str> = reps.iter().map(|r| r.local_date.as_str()).collect();
    let mut cursor = if dates.contains(today) {
        today_date
    } else {
        today_date - Duration::days(1)
    };
    let mut streak = 0u32;
    loop {
        let key = cursor.format("%Y-%m-%d").to_string();
        if !dates.contains(key.as_str()) {
            break;
        }
        streak += 1;
        if streak >= 36_500 {
            break;
        }
        cursor -= Duration::days(1);
    }
    streak
}

pub fn history_days(reps: &[Rep], limit: usize) -> Vec<DayHistory> {
    let mut dates: Vec<String> = reps.iter().map(|r| r.local_date.clone()).collect();
    dates.sort();
    dates.dedup();
    dates.reverse();
    dates
        .into_iter()
        .take(limit)
        .map(|date| {
            let mut day_reps: Vec<Rep> = reps
                .iter()
                .filter(|r| r.local_date == date)
                .cloned()
                .collect();
            day_reps.sort_by_key(|r| r.started_at_ms);
            day_reps.reverse();
            let count = day_reps.len() as u32;
            let total_ms = day_reps.iter().map(|r| r.duration_ms).sum();
            DayHistory {
                date,
                count,
                total_ms,
                reps: day_reps,
            }
        })
        .collect()
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// CSV：带 UTF-8 BOM 由调用方添加，Excel 才能正确识别中文。
pub fn to_csv(reps: &[Rep]) -> String {
    let mut out = String::from("日期,时间,时长(秒),时长(毫秒),开始时间戳(ms)\n");
    let mut sorted: Vec<&Rep> = reps.iter().collect();
    sorted.sort_by_key(|r| r.started_at_ms);
    for rep in sorted {
        let (date, time) = match Local.timestamp_millis_opt(rep.started_at_ms).single() {
            Some(dt) => (
                dt.format("%Y-%m-%d").to_string(),
                dt.format("%H:%M:%S").to_string(),
            ),
            None => ("1970-01-01".to_string(), "00:00:00".to_string()),
        };
        out.push_str(&format!(
            "{},{},{:.1},{},{}\n",
            csv_escape(&date),
            csv_escape(&time),
            rep.duration_ms as f64 / 1000.0,
            rep.duration_ms,
            rep.started_at_ms
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(date: &str, ms: u64) -> Rep {
        Rep {
            id: uuid::Uuid::new_v4().to_string(),
            started_at_ms: 0,
            duration_ms: ms,
            local_date: date.to_string(),
            tz_offset_min: 480,
        }
    }

    #[test]
    fn aggregates_by_day() {
        let reps = vec![rep("2026-11-12", 3000), rep("2026-11-13", 5000), rep("2026-11-13", 2000)];
        let today = day_stat(&reps, "2026-11-13");
        assert_eq!(today.count, 2);
        assert_eq!(today.total_ms, 7000);
    }

    #[test]
    fn streak_counts_consecutive_days() {
        let reps = vec![
            rep("2026-11-11", 1000),
            rep("2026-11-12", 1000),
            rep("2026-11-13", 1000),
        ];
        assert_eq!(streak_days(&reps, "2026-11-13"), 3);
        // 今天还没练，连续从昨天算
        assert_eq!(streak_days(&reps, "2026-11-14"), 3);
        // 断了一天
        assert_eq!(streak_days(&reps, "2026-11-15"), 0);
    }

    #[test]
    fn last_seven_days_includes_empty_days() {
        let reps = vec![rep("2026-11-13", 4000)];
        let days = last_n_days(&reps, "2026-11-13", 7);
        assert_eq!(days.len(), 7);
        assert_eq!(days[6].date, "2026-11-13");
        assert_eq!(days[6].count, 1);
        assert_eq!(days[0].date, "2026-11-07");
        assert_eq!(days[0].count, 0);
    }

    #[test]
    fn csv_has_header_and_rows() {
        let mut r = rep("2026-11-13", 5200);
        r.started_at_ms = 1_763_000_000_000;
        let csv = to_csv(&[r]);
        assert!(csv.starts_with("日期,时间"));
        assert!(csv.contains("5.2,5200"));
        assert_eq!(csv.lines().count(), 2);
    }

    #[test]
    fn csv_escapes_quotes() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
    }
}
