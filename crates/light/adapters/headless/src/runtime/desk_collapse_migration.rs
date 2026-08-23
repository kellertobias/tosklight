//! Choosing the one Programmer a desk keeps, out of however many it used to have.
//!
//! A desk written before the collapse can hold a Programmer per user, each under its own session.
//! Restoring them all would silently merge divergent work — one operator's values landing on top
//! of another's with nothing said about it — which is exactly what must not happen to a show.
//!
//! So one is chosen by a stated rule, the rest are written out where they can be recovered, and
//! the desk says in its log what it did.

use light_show::PersistedSession;
use serde::Serialize;
use std::path::Path;

/// What the collapse decided, and what it set aside.
pub(super) struct DeskCollapse {
    /// The persisted session whose Programmer the desk keeps operating. `None` when the desk has
    /// nothing persisted, which is the ordinary case for a fresh installation.
    pub(super) canonical: Option<PersistedSession>,
    /// Every other persisted Programmer, in the order they were superseded.
    pub(super) superseded: Vec<PersistedSession>,
}

impl DeskCollapse {
    /// Decide which persisted Programmer the desk keeps.
    ///
    /// **The policy: the Programmer touched most recently wins.** A desk should wake up holding
    /// what its operator last worked on. `persisted_sessions` is already ordered by `updated_at`,
    /// so the last row is the most recent; a tie is broken by session id so the same database
    /// always produces the same answer rather than depending on row order.
    pub(super) fn decide(mut sessions: Vec<PersistedSession>) -> Self {
        sessions.sort_by(|left, right| {
            left.updated_at
                .cmp(&right.updated_at)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });
        let canonical = sessions.pop();
        // Most recently superseded first, so a report reads newest-to-oldest like a log.
        sessions.reverse();
        Self {
            canonical,
            superseded: sessions,
        }
    }

    /// Whether this desk actually held more than one Programmer.
    pub(super) fn superseded_anything(&self) -> bool {
        !self.superseded.is_empty()
    }
}

/// How many superseded Programmers are written out whole.
///
/// A desk that has been running for months accumulates a persisted session per window that ever
/// connected — on a real development desk, over a thousand, and a Programmer holding a rig's worth
/// of values is a couple of hundred kilobytes. Copying every one produced a report of well over a
/// gigabyte, which is not a backup so much as a way to fill somebody's disk during startup.
///
/// So the report always *lists* every superseded Programmer, and copies the content of the most
/// recent few that actually hold something. Those are the ones an operator could plausibly want
/// back; the rest are recorded with what they held so nothing is silently unaccounted for.
const WHOLE_COPIES: usize = 20;

#[derive(Serialize)]
struct CollapseReport<'a> {
    collapsed_at: String,
    policy: &'a str,
    /// What this report copied and what it only listed.
    completeness: String,
    kept: ReportedProgrammer<'a>,
    superseded: Vec<ReportedProgrammer<'a>>,
}

#[derive(Serialize)]
struct ReportedProgrammer<'a> {
    session_id: uuid::Uuid,
    user_id: uuid::Uuid,
    updated_at: &'a str,
    /// What this Programmer held, so a listed-only entry still says whether it mattered.
    held: Held,
    /// The Programmer exactly as it was stored. Absent when only listed; see `completeness`.
    #[serde(skip_serializing_if = "Option::is_none")]
    programmer: Option<serde_json::Value>,
}

/// A count of what a stored Programmer held, read without interpreting it.
#[derive(Default, Serialize)]
struct Held {
    fixture_values: usize,
    group_values: usize,
    preload_values: usize,
    selected_fixtures: usize,
    command_line: bool,
}

impl Held {
    fn read(programmer: &serde_json::Value) -> Self {
        let count = |key: &str| {
            programmer
                .get(key)
                .map(|value| match value {
                    serde_json::Value::Array(items) => items.len(),
                    serde_json::Value::Object(entries) => entries.len(),
                    _ => 0,
                })
                .unwrap_or_default()
        };
        Self {
            fixture_values: count("values"),
            group_values: count("group_values"),
            preload_values: count("preload_pending") + count("preload_active"),
            selected_fixtures: count("selected"),
            command_line: programmer
                .get("command_line")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| !text.trim().is_empty()),
        }
    }

    /// Whether there is anything here somebody could want back.
    const fn is_anything(&self) -> bool {
        self.fixture_values > 0
            || self.group_values > 0
            || self.preload_values > 0
            || self.selected_fixtures > 0
            || self.command_line
    }
}

fn stored_programmer(session: &PersistedSession) -> serde_json::Value {
    serde_json::from_str(&session.programmer_json)
        .unwrap_or_else(|_| serde_json::Value::String(session.programmer_json.clone()))
}

fn reported<'a>(session: &'a PersistedSession, whole: bool) -> ReportedProgrammer<'a> {
    let programmer = stored_programmer(session);
    let held = Held::read(&programmer);
    ReportedProgrammer {
        session_id: session.id.0,
        user_id: session.user_id.0,
        updated_at: &session.updated_at,
        held,
        programmer: whole.then_some(programmer),
    }
}

/// Write the superseded Programmers where an operator can get them back.
///
/// Written before anything is restored, and a failure to write is a failure to collapse: losing
/// somebody's programming quietly is worse than refusing to start.
pub(super) fn write_collapse_report(
    data_dir: &Path,
    collapse: &DeskCollapse,
    now: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let Some(canonical) = collapse.canonical.as_ref() else {
        anyhow::bail!("a collapse with nothing to keep should not be reported")
    };
    let directory = data_dir.join("backups");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("desk-collapse-{}.json", now.replace(':', "-")));
    let mut copied = 0usize;
    let superseded = collapse
        .superseded
        .iter()
        .map(|session| {
            let holds_anything = Held::read(&stored_programmer(session)).is_anything();
            let whole = holds_anything && copied < WHOLE_COPIES;
            if whole {
                copied += 1;
            }
            reported(session, whole)
        })
        .collect::<Vec<_>>();
    let listed_only = superseded.len() - copied;
    let report = CollapseReport {
        collapsed_at: now.to_owned(),
        policy: "the Programmer touched most recently is kept; ties are broken by session id",
        completeness: format!(
            "{copied} superseded Programmers were copied whole, newest first; {listed_only} more \
             are listed with what they held but not copied"
        ),
        kept: reported(canonical, true),
        superseded,
    };
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{SessionId, UserId};

    fn session(id: u128, user: u128, updated_at: &str) -> PersistedSession {
        programmed_session(id, user, updated_at, 1)
    }

    /// A persisted session holding `values` retained fixture values.
    fn programmed_session(
        id: u128,
        user: u128,
        updated_at: &str,
        values: usize,
    ) -> PersistedSession {
        let held = (0..values)
            .map(|index| serde_json::json!({ "fixture_id": index }))
            .collect::<Vec<_>>();
        PersistedSession {
            id: SessionId(uuid::Uuid::from_u128(id)),
            user_id: UserId(uuid::Uuid::from_u128(user)),
            token: format!("token-{id}"),
            programmer_json: serde_json::json!({
                "session_id": uuid::Uuid::from_u128(id),
                "values": held,
            })
            .to_string(),
            connected: false,
            updated_at: updated_at.to_owned(),
        }
    }

    #[test]
    fn a_desk_with_nothing_persisted_has_nothing_to_choose() {
        let collapse = DeskCollapse::decide(Vec::new());
        assert!(collapse.canonical.is_none());
        assert!(!collapse.superseded_anything());
    }

    #[test]
    fn a_desk_with_one_programmer_keeps_it_and_supersedes_nothing() {
        let only = session(1, 10, "2026-08-01T00:00:00Z");
        let collapse = DeskCollapse::decide(vec![only.clone()]);
        assert!(!collapse.superseded_anything());
        assert_eq!(collapse.canonical.unwrap().id, only.id);
    }

    #[test]
    fn the_programmer_touched_most_recently_is_the_one_the_desk_keeps() {
        let collapse = DeskCollapse::decide(vec![
            session(1, 10, "2026-08-01T00:00:00Z"),
            session(2, 20, "2026-08-03T00:00:00Z"),
            session(3, 30, "2026-08-02T00:00:00Z"),
        ]);
        assert_eq!(
            collapse.canonical.unwrap().user_id.0,
            uuid::Uuid::from_u128(20)
        );
        assert_eq!(
            collapse
                .superseded
                .iter()
                .map(|session| session.user_id.0)
                .collect::<Vec<_>>(),
            vec![uuid::Uuid::from_u128(30), uuid::Uuid::from_u128(10)],
            "superseded Programmers are reported newest first"
        );
    }

    #[test]
    fn the_same_database_always_collapses_the_same_way() {
        let tied = || {
            vec![
                session(2, 20, "2026-08-03T00:00:00Z"),
                session(1, 10, "2026-08-03T00:00:00Z"),
            ]
        };
        let first = DeskCollapse::decide(tied());
        let mut reversed = tied();
        reversed.reverse();
        let second = DeskCollapse::decide(reversed);
        assert_eq!(
            first.canonical.unwrap().id,
            second.canonical.unwrap().id,
            "a tie must not be decided by the order rows came back in"
        );
    }

    #[test]
    fn a_superseded_programmer_is_written_out_where_it_can_be_recovered() {
        let directory = scratch_directory();
        let collapse = DeskCollapse::decide(vec![
            session(1, 10, "2026-08-01T00:00:00Z"),
            session(2, 20, "2026-08-03T00:00:00Z"),
        ]);

        let path = write_collapse_report(&directory, &collapse, "2026-08-23T12:00:00Z").unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written["kept"]["user_id"],
            uuid::Uuid::from_u128(20).to_string()
        );
        assert_eq!(written["superseded"].as_array().unwrap().len(), 1);
        assert_eq!(
            written["superseded"][0]["programmer"]["session_id"],
            uuid::Uuid::from_u128(1).to_string(),
            "the superseded Programmer is kept whole, not summarised"
        );
        assert!(
            written["policy"]
                .as_str()
                .unwrap()
                .contains("most recently")
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_desk_with_a_long_history_reports_all_of_it_without_copying_all_of_it() {
        // A desk that has been running for months accumulates a persisted session per window that
        // ever connected. Copying every Programmer produced a report over a gigabyte on a real
        // development desk, so the report lists them all and copies the recent ones that held
        // something.
        let directory = scratch_directory();
        let mut sessions = (0..60)
            .map(|index| {
                programmed_session(
                    index + 1,
                    10,
                    &format!("2026-08-01T00:{:02}:00Z", index),
                    40,
                )
            })
            .collect::<Vec<_>>();
        // An empty Programmer has nothing to recover, so it is listed rather than copied.
        sessions.push(programmed_session(999, 10, "2026-08-01T00:00:30Z", 0));
        let collapse = DeskCollapse::decide(sessions);

        let path = write_collapse_report(&directory, &collapse, "2026-08-23T12:00:00Z").unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let superseded = written["superseded"].as_array().unwrap();
        assert_eq!(
            superseded.len(),
            60,
            "every superseded Programmer is listed"
        );
        let copied = superseded
            .iter()
            .filter(|entry| entry.get("programmer").is_some())
            .count();
        assert_eq!(copied, WHOLE_COPIES);
        // The empty one is listed with what it held, and not copied.
        let empty = superseded
            .iter()
            .find(|entry| entry["session_id"] == uuid::Uuid::from_u128(999).to_string())
            .expect("the empty Programmer is still accounted for");
        assert!(empty.get("programmer").is_none());
        assert_eq!(empty["held"]["fixture_values"], 0);
        // And the ones that were copied say how much they held.
        assert_eq!(superseded[0]["held"]["fixture_values"], 40);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn collapsing_a_desk_leaves_every_screen_configuration_alone() {
        // The collapse chooses a Programmer. Screens are presentation, not desk authority, so a
        // desk that held several Programmers keeps every screen exactly as it was — names,
        // layouts, playback layouts, Follow Main / Dedicated Page, display assignment, and the
        // Not Editable flag.
        let directory = scratch_directory();
        let store = light_show::DeskStore::open(&directory.join("desk.sqlite")).unwrap();
        let screen = light_show::ScreenConfiguration {
            id: uuid::Uuid::new_v4(),
            name: "Foyer repeater".into(),
            layout: serde_json::json!({"desks": [], "activeDeskId": ""}),
            show_dock: false,
            show_playbacks: true,
            playback_count: 6,
            playback_rows: 2,
            first_playback_slot: 3,
            page_mode: "independent".into(),
            show_page_controls: false,
            show_programmer: true,
            desired_open: true,
            display_id: Some("display-2".into()),
            bounds: Some(serde_json::json!({"x": 10, "y": 20, "width": 800, "height": 600})),
            fullscreen: true,
            playback_layout: None,
            content: light_show::ScreenContent::default(),
            not_editable: true,
        };
        let stored = store.put_screen(screen.clone()).unwrap();

        let collapse = DeskCollapse::decide(vec![
            session(1, 10, "2026-08-01T00:00:00Z"),
            session(2, 20, "2026-08-03T00:00:00Z"),
        ]);
        write_collapse_report(&directory, &collapse, "2026-08-23T12:00:00Z").unwrap();

        let after = store
            .screen(screen.id)
            .unwrap()
            .expect("the screen survives");
        assert_eq!(after.name, stored.name);
        assert_eq!(after.layout, stored.layout);
        assert_eq!(after.playback_count, stored.playback_count);
        assert_eq!(after.playback_rows, stored.playback_rows);
        assert!(after.not_editable);
        assert_eq!(after.page_mode, "independent");
        assert_eq!(after.first_playback_slot, 3);
        assert_eq!(after.display_id.as_deref(), Some("display-2"));
        assert!(after.fullscreen);
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A scratch directory under the repository's temporary root when one is set.
    fn scratch_directory() -> std::path::PathBuf {
        let root = std::env::var_os("LIGHT_TMP_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = root.join(format!("desk-collapse-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }
}
