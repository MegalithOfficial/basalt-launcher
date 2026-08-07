use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};

use crate::error::Result;

use super::{DayBucket, Db, InstancePlayStat, LoaderPlayStat, PlaySession, PlayStats};

const VANILLA: &str = "vanilla";
pub const SESSIONS_PER_PAGE: usize = 50;

pub(super) struct LiveInstance {
    pub name: String,
    pub playtime_secs: i64,
    pub last_played_at: Option<i64>,
}

type LiveInstances = HashMap<String, LiveInstance>;

fn local_date(timestamp: i64) -> Option<NaiveDate> {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|moment| moment.date_naive())
}

fn local_hour(timestamp: i64) -> Option<u32> {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|moment| moment.hour())
}

fn streaks(active: &BTreeSet<NaiveDate>, today: NaiveDate) -> (i64, i64) {
    let mut longest = 0i64;
    let mut run = 0i64;
    let mut previous: Option<NaiveDate> = None;
    for day in active {
        run = match previous {
            Some(last) if *day == last + Duration::days(1) => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        previous = Some(*day);
    }

    let mut current = 0i64;
    let mut cursor = if active.contains(&today) {
        today
    } else if active.contains(&(today - Duration::days(1))) {
        today - Duration::days(1)
    } else {
        return (0, longest);
    };
    while active.contains(&cursor) {
        current += 1;
        cursor -= Duration::days(1);
    }

    (current, longest)
}

fn build_stats(
    sessions: Vec<PlaySession>,
    lifetime_secs: i64,
    live: LiveInstances,
    days: Option<u32>,
    page: Option<u32>,
    today: NaiveDate,
) -> PlayStats {
    let tracked_since = sessions.first().map(|session| session.started_at);
    let active: BTreeSet<NaiveDate> = sessions
        .iter()
        .filter_map(|session| local_date(session.started_at))
        .collect();
    let (current_streak_days, longest_streak_days) = streaks(&active, today);

    let window_start = days.map(|count| today - Duration::days(i64::from(count.saturating_sub(1))));
    let windowed: Vec<&PlaySession> = sessions
        .iter()
        .filter(|session| match window_start {
            None => true,
            Some(start) => local_date(session.started_at).is_some_and(|date| date >= start),
        })
        .collect();

    let mut per_instance: HashMap<String, InstancePlayStat> = live
        .iter()
        .filter(|(_, entry)| entry.playtime_secs > 0)
        .map(|(id, entry)| {
            (
                id.clone(),
                InstancePlayStat {
                    instance_id: id.clone(),
                    name: entry.name.clone(),
                    secs: 0,
                    sessions: 0,
                    crashes: 0,
                    last_played_at: entry.last_played_at,
                    lifetime_secs: entry.playtime_secs,
                    deleted: false,
                },
            )
        })
        .collect();

    let mut buckets: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    let mut hourly = vec![0i64; 24];
    let mut weekday = vec![0i64; 7];
    let mut per_loader: BTreeMap<String, LoaderPlayStat> = BTreeMap::new();
    let mut window_secs = 0i64;
    let mut crash_count = 0i64;
    let mut longest_session_secs = 0i64;

    for session in &windowed {
        window_secs += session.played_secs;
        if session.crashed {
            crash_count += 1;
        }
        longest_session_secs = longest_session_secs.max(session.played_secs);

        if let Some(date) = local_date(session.started_at) {
            let entry = buckets.entry(date).or_insert((0, 0));
            entry.0 += session.played_secs;
            entry.1 += 1;
            weekday[date.weekday().num_days_from_monday() as usize] += session.played_secs;
        }
        if let Some(hour) = local_hour(session.started_at) {
            hourly[hour as usize] += session.played_secs;
        }

        let stat = per_instance
            .entry(session.instance_id.clone())
            .or_insert_with(|| InstancePlayStat {
                instance_id: session.instance_id.clone(),
                name: session.instance_name.clone(),
                secs: 0,
                sessions: 0,
                crashes: 0,
                last_played_at: None,
                lifetime_secs: 0,
                deleted: true,
            });
        stat.secs += session.played_secs;
        stat.sessions += 1;
        if session.crashed {
            stat.crashes += 1;
        }
        stat.last_played_at = Some(stat.last_played_at.unwrap_or(0).max(session.ended_at));

        let loader = session
            .loader
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(VANILLA)
            .to_string();
        let bucket = per_loader
            .entry(loader.clone())
            .or_insert_with(|| LoaderPlayStat {
                loader,
                secs: 0,
                sessions: 0,
            });
        bucket.secs += session.played_secs;
        bucket.sessions += 1;
    }

    for stat in per_instance.values_mut() {
        if let Some(entry) = live.get(&stat.instance_id) {
            stat.name = entry.name.clone();
            stat.lifetime_secs = entry.playtime_secs;
            stat.deleted = false;
        }
    }

    let first_day = window_start
        .or_else(|| buckets.keys().next().copied())
        .unwrap_or(today);
    let mut daily = Vec::new();
    let mut cursor = first_day.min(today);
    while cursor <= today {
        let (secs, count) = buckets.get(&cursor).copied().unwrap_or((0, 0));
        daily.push(DayBucket {
            date: cursor.format("%Y-%m-%d").to_string(),
            secs,
            sessions: count,
        });
        cursor += Duration::days(1);
    }

    let busiest_day = daily
        .iter()
        .filter(|day| day.secs > 0)
        .max_by_key(|day| day.secs)
        .cloned();
    let active_days = daily.iter().filter(|day| day.secs > 0).count() as i64;
    let session_count = windowed.len() as i64;
    let average_session_secs = if session_count > 0 {
        window_secs / session_count
    } else {
        0
    };

    let mut instances: Vec<InstancePlayStat> = per_instance.into_values().collect();
    instances.sort_by(|a, b| {
        b.secs
            .cmp(&a.secs)
            .then_with(|| b.lifetime_secs.cmp(&a.lifetime_secs))
            .then_with(|| a.name.cmp(&b.name))
    });
    let mut loaders: Vec<LoaderPlayStat> = per_loader.into_values().collect();
    loaders.sort_by(|a, b| b.secs.cmp(&a.secs).then_with(|| a.loader.cmp(&b.loader)));

    let newest = windowed.iter().rev().map(|session| (*session).clone());
    let recent: Vec<PlaySession> = match page {
        None => newest.collect(),
        Some(page) => newest
            .skip(page as usize * SESSIONS_PER_PAGE)
            .take(SESSIONS_PER_PAGE)
            .collect(),
    };

    PlayStats {
        lifetime_secs,
        tracked_since,
        window_days: days,
        window_secs,
        session_count,
        crash_count,
        longest_session_secs,
        average_session_secs,
        active_days,
        current_streak_days,
        longest_streak_days,
        busiest_day,
        daily,
        hourly,
        weekday,
        instances,
        loaders,
        recent,
        recent_total: windowed.len() as i64,
        recent_page: page,
    }
}

impl Db {
    pub fn play_stats(&self, days: Option<u32>, page: Option<u32>) -> Result<PlayStats> {
        let sessions = self.play_sessions()?;
        let (lifetime_secs, live) = self.instance_playtime_totals()?;
        Ok(build_stats(
            sessions,
            lifetime_secs,
            live,
            days,
            page,
            Local::now().date_naive(),
        ))
    }

    pub fn current_streak_days(&self) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT started_at FROM play_sessions")?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        let mut active = BTreeSet::new();
        for row in rows {
            if let Some(date) = local_date(row?) {
                active.insert(date);
            }
        }
        Ok(streaks(&active, Local::now().date_naive()).0)
    }

    fn play_sessions(&self) -> Result<Vec<PlaySession>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, instance_id, instance_name, started_at, ended_at, played_secs,
                    crashed, version_id, loader
             FROM play_sessions ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PlaySession {
                id: row.get(0)?,
                instance_id: row.get(1)?,
                instance_name: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                played_secs: row.get(5)?,
                crashed: row.get::<_, i64>(6)? != 0,
                version_id: row.get(7)?,
                loader: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn instance_playtime_totals(&self) -> Result<(i64, LiveInstances)> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, playtime_secs, last_played_at FROM instances")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                LiveInstance {
                    name: row.get(1)?,
                    playtime_secs: row.get(2)?,
                    last_played_at: row.get(3)?,
                },
            ))
        })?;
        let mut total = 0i64;
        let mut live = LiveInstances::new();
        for row in rows {
            let (id, entry) = row?;
            total += entry.playtime_secs;
            live.insert(id, entry);
        }
        Ok((total, live))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ActiveRun;

    fn session(id: i64, instance: &str, started_at: i64, secs: i64, crashed: bool) -> PlaySession {
        PlaySession {
            id,
            instance_id: instance.to_string(),
            instance_name: format!("{instance} name"),
            started_at,
            ended_at: started_at + secs,
            played_secs: secs,
            crashed,
            version_id: Some("1.20.1".into()),
            loader: Some("fabric".into()),
        }
    }

    fn at(date: &str, hour: u32) -> i64 {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        Local
            .from_local_datetime(&day.and_hms_opt(hour, 0, 0).unwrap())
            .single()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn streaks_count_consecutive_local_days() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let active: BTreeSet<NaiveDate> = ["2026-07-20", "2026-08-02", "2026-08-03", "2026-08-04"]
            .iter()
            .map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap())
            .collect();
        assert_eq!(streaks(&active, today), (3, 3));
    }

    #[test]
    fn streak_survives_a_session_yesterday_but_none_today() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let active: BTreeSet<NaiveDate> = ["2026-08-02", "2026-08-03"]
            .iter()
            .map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap())
            .collect();
        assert_eq!(streaks(&active, today), (2, 2));
    }

    #[test]
    fn window_limits_buckets_and_fills_quiet_days() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let sessions = vec![
            session(1, "old", at("2026-06-01", 20), 3600, false),
            session(2, "a", at("2026-08-02", 20), 1800, false),
            session(3, "a", at("2026-08-04", 9), 5400, true),
        ];
        let mut live = LiveInstances::new();
        live.insert(
            "a".to_string(),
            LiveInstance {
                name: "Skyblock".to_string(),
                playtime_secs: 90_000,
                last_played_at: Some(at("2026-08-04", 11)),
            },
        );

        let stats = build_stats(sessions, 90_000, live, Some(3), None, today);

        assert_eq!(stats.daily.len(), 3);
        assert_eq!(stats.daily[0].date, "2026-08-02");
        assert_eq!(stats.daily[1].secs, 0);
        assert_eq!(stats.window_secs, 7200);
        assert_eq!(stats.session_count, 2);
        assert_eq!(stats.crash_count, 1);
        assert_eq!(stats.longest_session_secs, 5400);
        assert_eq!(stats.active_days, 2);
        assert_eq!(stats.lifetime_secs, 90_000);
        assert_eq!(stats.instances.len(), 1);
        assert_eq!(stats.instances[0].name, "Skyblock");
        assert!(!stats.instances[0].deleted);
        assert_eq!(stats.busiest_day.unwrap().date, "2026-08-04");
    }

    #[test]
    fn instances_with_lifetime_playtime_appear_before_any_session_exists() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let mut live = LiveInstances::new();
        live.insert(
            "quiet".to_string(),
            LiveInstance {
                name: "Create".to_string(),
                playtime_secs: 7200,
                last_played_at: Some(1_700_000_000),
            },
        );
        live.insert(
            "never".to_string(),
            LiveInstance {
                name: "Fresh install".to_string(),
                playtime_secs: 0,
                last_played_at: None,
            },
        );

        let stats = build_stats(Vec::new(), 7200, live, Some(30), None, today);

        assert_eq!(stats.instances.len(), 1);
        assert_eq!(stats.instances[0].name, "Create");
        assert_eq!(stats.instances[0].lifetime_secs, 7200);
        assert_eq!(stats.instances[0].secs, 0);
        assert_eq!(stats.instances[0].last_played_at, Some(1_700_000_000));
    }

    #[test]
    fn deleted_instances_keep_their_recorded_name() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let sessions = vec![session(1, "gone", at("2026-08-03", 12), 600, false)];
        let stats = build_stats(sessions, 0, LiveInstances::new(), None, None, today);
        assert_eq!(stats.instances[0].name, "gone name");
        assert!(stats.instances[0].deleted);
        assert_eq!(stats.instances[0].lifetime_secs, 0);
    }

    #[test]
    fn sessions_land_in_their_local_hour_and_weekday() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let sessions = vec![session(1, "a", at("2026-08-03", 21), 1200, false)];
        let stats = build_stats(sessions, 0, LiveInstances::new(), None, None, today);
        assert_eq!(stats.hourly[21], 1200);
        assert_eq!(stats.weekday[0], 1200);
        assert_eq!(stats.loaders[0].loader, "fabric");
    }

    #[test]
    fn record_playtime_appends_a_session_and_bumps_the_lifetime_total() {
        let db = Db::open_in_memory().unwrap();
        db.0.lock()
            .unwrap()
            .execute(
                "INSERT INTO instances (id, name, version_id, created_at, playtime_secs, loader)
                 VALUES ('i1', 'Skyblock', '1.20.1', '2026-08-01', 100, 'fabric')",
                [],
            )
            .unwrap();

        let started_at = Local::now().timestamp() - 3600;
        db.save_active_run(&ActiveRun {
            running_id: "run-1".into(),
            instance_id: "i1".into(),
            pid: 42,
            process_started_at: 1,
            started_at,
            checkpointed_at: started_at,
        })
        .unwrap();
        assert!(db
            .finalize_active_run("run-1", started_at + 3600, true)
            .unwrap());

        let stats = db.play_stats(None, None).unwrap();
        assert_eq!(stats.session_count, 1);
        assert_eq!(stats.window_secs, 3600);
        assert_eq!(stats.crash_count, 1);
        assert_eq!(stats.lifetime_secs, 3700);
        assert_eq!(stats.instances[0].name, "Skyblock");
        assert!(!stats.instances[0].deleted);
        assert_eq!(stats.recent[0].loader.as_deref(), Some("fabric"));
        assert!(stats.recent[0].crashed);
    }

    #[test]
    fn a_page_slices_the_newest_sessions_and_no_page_returns_everything() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let total = SESSIONS_PER_PAGE + 20;
        let sessions: Vec<PlaySession> = (0..total)
            .map(|index| {
                session(
                    index as i64,
                    "a",
                    at("2026-08-03", 10) + index as i64,
                    600,
                    false,
                )
            })
            .collect();

        let everything = build_stats(sessions.clone(), 0, LiveInstances::new(), None, None, today);
        assert_eq!(everything.recent.len(), total);
        assert_eq!(everything.recent_total, total as i64);
        assert_eq!(everything.recent_page, None);

        let first = build_stats(
            sessions.clone(),
            0,
            LiveInstances::new(),
            None,
            Some(0),
            today,
        );
        assert_eq!(first.recent.len(), SESSIONS_PER_PAGE);
        assert_eq!(first.recent_total, total as i64);
        assert_eq!(first.recent[0].id, everything.recent[0].id);

        let second = build_stats(sessions, 0, LiveInstances::new(), None, Some(1), today);
        assert_eq!(second.recent.len(), 20);
        assert_eq!(second.recent[0].id, everything.recent[SESSIONS_PER_PAGE].id);

        let past_the_end = build_stats(Vec::new(), 0, LiveInstances::new(), None, Some(9), today);
        assert!(past_the_end.recent.is_empty());
    }

    #[test]
    fn empty_history_reports_zeroes_without_panicking() {
        let today = NaiveDate::parse_from_str("2026-08-04", "%Y-%m-%d").unwrap();
        let stats = build_stats(Vec::new(), 0, LiveInstances::new(), Some(7), None, today);
        assert_eq!(stats.daily.len(), 7);
        assert_eq!(stats.session_count, 0);
        assert_eq!(stats.average_session_secs, 0);
        assert!(stats.busiest_day.is_none());
        assert!(stats.tracked_since.is_none());
    }
}
