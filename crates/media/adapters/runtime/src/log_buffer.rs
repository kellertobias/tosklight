//! A bounded window of recent log records, held in memory.
//!
//! An operator diagnosing a machine at a venue has no terminal and no log file they can reach. What
//! they have is a browser — so every record the process emits is also kept here, and the log viewer
//! reads it.
//!
//! Two rules shape this. It is *bounded*: a server running all week must not grow a log until it
//! runs out of memory, so the oldest record is discarded and the count of discards is published
//! rather than hidden. And it never blocks an emitter for long: capturing a record is a short lock
//! around a push, which is what `tracing`'s own subscriber does, and no real-time path emits
//! directly — the render thread, the audio callback, and the packet parsers publish into channels
//! and let a worker log.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use media_http::{LogEntry, LogPage, LogQuery};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// How many records the window holds.
///
/// Enough to cover a startup, a show, and the failure someone is looking for; small enough that
/// the whole window is a few megabytes on the smallest machine Media supports.
pub const CAPACITY: usize = 2_000;

/// The window, shared between the subscriber that fills it and the API that reads it.
#[derive(Debug, Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<Inner>>,
    started: Instant,
}

#[derive(Debug)]
struct Inner {
    entries: VecDeque<LogEntry>,
    next_sequence: u64,
    dropped: u64,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                entries: VecDeque::with_capacity(CAPACITY),
                // One-based, so a viewer holding cursor zero asks for everything.
                next_sequence: 1,
                dropped: 0,
            })),
            started: Instant::now(),
        }
    }

    /// Keeps one record, discarding the oldest when the window is full.
    fn remember(&self, level: &str, target: &str, message: String) {
        let millis_since_start = self.started.elapsed().as_millis() as u64;
        // A poisoned lock would mean a panic inside this function; dropping the record is better
        // than making every later log call panic too.
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let sequence = inner.next_sequence;
        inner.next_sequence += 1;
        if inner.entries.len() == CAPACITY {
            inner.entries.pop_front();
            inner.dropped += 1;
        }
        inner.entries.push_back(LogEntry {
            sequence,
            millis_since_start,
            level: level.to_owned(),
            target: target.to_owned(),
            message,
        });
    }

    /// The records a viewer asked for.
    pub fn page(&self, query: &LogQuery) -> LogPage {
        let Ok(inner) = self.inner.lock() else {
            return LogPage {
                capacity: CAPACITY,
                ..LogPage::default()
            };
        };
        let newest = inner.next_sequence.saturating_sub(1);
        // Absent means no filter; present but unrecognized is a filter nothing satisfies, which is
        // the honest answer to being asked for a level that does not exist.
        let threshold = query.level.as_deref().map(severity);

        let matching = inner.entries.iter().filter(|entry| {
            let severe_enough = match threshold {
                None => true,
                Some(None) => false,
                Some(Some(wanted)) => severity(&entry.level).is_some_and(|at| at <= wanted),
            };
            entry.sequence > query.after.unwrap_or(0) && severe_enough
        });
        // Without a cursor a viewer wants the *newest* window; with one it wants the oldest
        // records it has not seen, so that repeated reads walk forward rather than skip.
        let entries: Vec<LogEntry> = if query.after.is_some() {
            matching.take(query.limit).cloned().collect()
        } else {
            let all: Vec<&LogEntry> = matching.collect();
            all.iter()
                .rev()
                .take(query.limit)
                .rev()
                .map(|entry| (*entry).clone())
                .collect()
        };

        LogPage {
            entries,
            newest,
            dropped: inner.dropped,
            capacity: CAPACITY,
        }
    }

    /// The `tracing` layer that fills this window.
    pub fn layer<S: Subscriber>(&self) -> BufferLayer<S> {
        BufferLayer {
            buffer: self.clone(),
            subscriber: std::marker::PhantomData,
        }
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// How severe a level is, lowest number being most severe. `None` for anything unrecognized.
fn severity(level: &str) -> Option<u8> {
    match level {
        "error" => Some(0),
        "warn" => Some(1),
        "info" => Some(2),
        "debug" => Some(3),
        "trace" => Some(4),
        _ => None,
    }
}

fn label(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

/// Copies every emitted record into a [`LogBuffer`].
pub struct BufferLayer<S> {
    buffer: LogBuffer,
    subscriber: std::marker::PhantomData<fn(S)>,
}

impl<S: Subscriber> Layer<S> for BufferLayer<S> {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut message = Rendered::default();
        event.record(&mut message);
        self.buffer.remember(
            label(event.metadata().level()),
            event.metadata().target(),
            message.finish(),
        );
    }
}

/// One record's fields, rendered the way the terminal renders them.
///
/// The message first, then each remaining field as `name=value`, so a record in the viewer reads
/// the same as the one in a terminal an engineer is used to.
#[derive(Debug, Default)]
struct Rendered {
    message: String,
    fields: String,
}

impl Rendered {
    fn finish(self) -> String {
        if self.fields.is_empty() {
            return self.message;
        }
        if self.message.is_empty() {
            return self.fields;
        }
        format!("{} {}", self.message, self.fields)
    }

    fn push(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
            // A message is recorded as a debug value, which quotes a plain string. An operator
            // should not have to read those quotes.
            self.message = self
                .message
                .trim_matches('"')
                .replace("\\\"", "\"")
                .replace("\\n", "\n");
            return;
        }
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{}={value:?}", field.name());
    }
}

impl Visit for Rendered {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.push(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_owned();
            return;
        }
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{}={value}", field.name());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remember(buffer: &LogBuffer, level: &str, message: &str) {
        buffer.remember(level, "media_runtime", message.to_owned());
    }

    fn query(after: Option<u64>, level: Option<&str>, limit: usize) -> LogQuery {
        LogQuery {
            after,
            level: level.map(str::to_owned),
            limit,
        }
    }

    #[test]
    fn records_are_numbered_so_a_viewer_can_ask_for_what_is_new() {
        let buffer = LogBuffer::new();
        remember(&buffer, "info", "media server starting");
        remember(&buffer, "warn", "no audio input");

        let page = buffer.page(&query(None, None, 10));
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].sequence, 1);
        assert_eq!(page.entries[1].sequence, 2);
        assert_eq!(page.newest, 2);

        let since = buffer.page(&query(Some(1), None, 10));
        assert_eq!(since.entries.len(), 1);
        assert_eq!(since.entries[0].message, "no audio input");
    }

    #[test]
    fn the_window_is_bounded_and_says_how_much_it_discarded() {
        let buffer = LogBuffer::new();
        for index in 0..CAPACITY + 5 {
            remember(&buffer, "info", &format!("record {index}"));
        }

        let page = buffer.page(&query(None, None, CAPACITY));
        assert_eq!(page.entries.len(), CAPACITY);
        assert_eq!(page.dropped, 5, "a lost record is reported, never hidden");
        assert_eq!(page.entries[0].message, "record 5");
        assert_eq!(page.newest, (CAPACITY + 5) as u64);
    }

    #[test]
    fn a_level_filter_keeps_everything_at_least_as_severe() {
        let buffer = LogBuffer::new();
        remember(&buffer, "error", "cannot open the output window");
        remember(&buffer, "info", "library discovered");
        remember(&buffer, "warn", "no audio input");
        remember(&buffer, "debug", "frame prepared");

        let warnings = buffer.page(&query(None, Some("warn"), 10));
        let messages: Vec<&str> = warnings
            .entries
            .iter()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(
            messages,
            vec!["cannot open the output window", "no audio input"],
            "an error is worth seeing when warnings were asked for"
        );
    }

    #[test]
    fn a_first_read_gets_the_newest_records_and_a_cursor_read_walks_forward() {
        let buffer = LogBuffer::new();
        for index in 0..10 {
            remember(&buffer, "info", &format!("record {index}"));
        }

        let newest = buffer.page(&query(None, None, 3));
        assert_eq!(newest.entries[0].message, "record 7");
        assert_eq!(newest.entries[2].message, "record 9");

        let forward = buffer.page(&query(Some(0), None, 3));
        assert_eq!(
            forward.entries[0].message, "record 0",
            "a viewer catching up starts where it left off"
        );
        assert_eq!(forward.newest, 10, "and can tell it is still behind");
    }

    #[test]
    fn an_unrecognized_level_filter_matches_nothing_rather_than_everything() {
        let buffer = LogBuffer::new();
        remember(&buffer, "info", "library discovered");
        assert!(
            buffer
                .page(&query(None, Some("shouting"), 10))
                .entries
                .is_empty()
        );
    }

    #[test]
    fn a_records_fields_are_rendered_the_way_a_terminal_renders_them() {
        let rendered = Rendered {
            message: "library discovered".to_owned(),
            fields: "items=12".to_owned(),
        };
        assert_eq!(rendered.finish(), "library discovered items=12");

        let bare = Rendered {
            message: "media server stopping".to_owned(),
            fields: String::new(),
        };
        assert_eq!(bare.finish(), "media server stopping");
    }

    #[test]
    fn an_emitted_record_reaches_the_window_through_the_layer() {
        use tracing_subscriber::layer::SubscriberExt as _;

        let buffer = LogBuffer::new();
        let subscriber = tracing_subscriber::registry().with(buffer.layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(items = 12, "library discovered");
        });

        let page = buffer.page(&query(None, None, 10));
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].level, "warn");
        assert_eq!(page.entries[0].target, "media_runtime::log_buffer::tests");
        assert_eq!(
            page.entries[0].message, "library discovered items=12",
            "a structured field is readable rather than lost"
        );
    }
}
