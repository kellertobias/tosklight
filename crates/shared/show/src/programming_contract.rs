//! Portable-show programming-contract marker and the legacy-programming validator (TL-560).
//!
//! The marker is an ordinary portable-show metadata key, not a schema bump. `SHOW_SCHEMA_VERSION`
//! describes the SQLite layout, and `ShowStore::open` raises it in place on every older file, so
//! it cannot say which programming contract wrote the content. A metadata key is retained by
//! every reader and writer that does not own it, and an older build simply ignores it.
//!
//! The validator is dormant at contract 0: [`validate_show_programming_contract`] returns
//! immediately and never opens the file, so production behaviour is unchanged until the cutover
//! (TL-552) raises the runtime contract. At contract 1 or later it inspects the file read-only
//! (the original bytes are never migrated or rewritten) and rejects a show that still holds
//! legacy normalized Position (`pan`/`tilt`), legacy Color component (`color.red`, …) or
//! percentage Zoom (a `normalized`/`spread` value at `zoom`) programming, or whose marker names
//! a newer contract.

use crate::StoreError;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::{collections::BTreeSet, fmt, path::Path};

/// Portable-show metadata key. The value is the decimal programming contract of the writer.
pub const PROGRAMMING_CONTRACT_METADATA_KEY: &str = "light.programming_contract";

/// Object kinds that carry authored programming. Fixture profiles and patch records also use
/// `pan`/`tilt`/`color.*` as fixture-facing attribute names and are never inspected.
pub const PROGRAMMING_OBJECT_KINDS: &[&str] = &[
    "preset",
    "cue_list",
    "group",
    "dynamic",
    "playback",
    "playback_page",
];

/// Whole family a legacy scalar programming address belongs to.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyProgrammingFamily {
    Position,
    Color,
    /// A percentage (`normalized`/`spread`) at `zoom`. Semantic Zoom is an opening in degrees
    /// (`{"kind": "zoom", …}`) at the same address, so only the value kind tells them apart.
    Zoom,
}

impl LegacyProgrammingFamily {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Position => "normalized Position",
            Self::Color => "Color component",
            Self::Zoom => "percentage Zoom",
        }
    }
}

/// Programming addresses written before the semantic contract. Wheel slots (`color.wheel*`,
/// a native indexed path like Gobo), Media color (`color.tint`) and every non-family attribute
/// (intensity, beam, gobo, `position.movement`, Focus, …) are deliberately absent. `zoom` is
/// legacy only with a percentage value (TL-552 owner decision, plan §13
/// "normalized-position/zoom"); see [`legacy_value_kind`].
pub fn legacy_programming_family(attribute: &str) -> Option<LegacyProgrammingFamily> {
    match attribute {
        "zoom" => Some(LegacyProgrammingFamily::Zoom),
        "pan" | "tilt" | "pan.continuous" | "tilt.continuous" => {
            Some(LegacyProgrammingFamily::Position)
        }
        "color.red"
        | "color.green"
        | "color.blue"
        | "color.white"
        | "color.amber"
        | "color.uv"
        | "color.lime"
        | "color.mint"
        | "color.indigo"
        | "color.cyan"
        | "color.magenta"
        | "color.yellow"
        | "color.cold_white"
        | "color.warm_white"
        | "color.hue"
        | "color.saturation"
        | "color.brightness"
        | "color.temperature"
        | "color.white_blend"
        | "color.duv"
        | "color.relative_output" => Some(LegacyProgrammingFamily::Color),
        _ => None,
    }
}

/// One legacy address inside one stored record.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LegacyProgrammingFinding {
    pub kind: String,
    pub id: String,
    pub attribute: String,
    pub family: LegacyProgrammingFamily,
}

/// Collects every legacy address in one stored programming body: a scalar value map entry
/// (`{"pan": {"kind": "normalized", …}}`, as in Preset/Group/Programmer value maps) or a row or
/// lane addressed by `"attribute": "pan"` (Cue changes, legacy scalar Dynamic lanes).
pub fn legacy_programming_attributes(body: &Value) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    collect(body, &mut found);
    found
}

/// Whether an authored value of this `kind` at `attribute` is legacy. Position and Color
/// component addresses are legacy with any value; `zoom` only with a percentage, because
/// semantic Zoom (`kind: "zoom"`, degrees) shares the address.
fn legacy_value_kind(attribute: &str, kind: Option<&str>) -> bool {
    match legacy_programming_family(attribute) {
        Some(LegacyProgrammingFamily::Zoom) => {
            kind.is_none_or(|kind| matches!(kind, "normalized" | "spread"))
        }
        Some(_) => true,
        None => false,
    }
}

/// Live-write counterpart of the stored rule (TL-552 follow-up): the family of a legacy address
/// carrying a value whose serde `kind` is `value_kind`, or `None` when the stored validator
/// would accept it. Every live writer at contract ≥ 1 applies exactly this rule, so nothing it
/// accepts can make the show or the stored Programmer fail the load-time validator.
pub fn legacy_programming_address(
    attribute: &str,
    value_kind: Option<&str>,
) -> Option<LegacyProgrammingFamily> {
    legacy_value_kind(attribute, value_kind)
        .then(|| legacy_programming_family(attribute))
        .flatten()
}

/// [`legacy_programming_address`] for one authored [`light_core::AttributeValue`].
pub fn legacy_attribute_value(
    attribute: &str,
    value: &light_core::AttributeValue,
) -> Option<LegacyProgrammingFamily> {
    use light_core::AttributeValue as V;
    // The serde `kind` of the variants the stored rule distinguishes; every other kind is only
    // legacy at an address that is legacy with any value.
    let kind = match value {
        V::Normalized(_) => "normalized",
        V::Spread(_) => "spread",
        _ => "other",
    };
    legacy_programming_address(attribute, Some(kind))
}

/// The actionable refusal for one legacy live write at `supported` ≥ 1.
pub fn legacy_live_write_message(
    attribute: &str,
    family: LegacyProgrammingFamily,
    supported: u16,
) -> String {
    let instead = match family {
        LegacyProgrammingFamily::Position => {
            "Program Position on `position` instead: Pan and Tilt in degrees (an `apply_intent` \
             with `component_edits`), a Position Preset, or Aim"
        }
        LegacyProgrammingFamily::Color => {
            "Program Color on `color` instead: the Color controls (an `apply_intent` with \
             `component_edits`) or a Color Preset"
        }
        LegacyProgrammingFamily::Zoom => {
            "Program Zoom as an opening in degrees instead (a `zoom` value, or an \
             `apply_intent` with `component_edits`)"
        }
    };
    format!(
        "`{attribute}` is {} programming from before semantic programming contract {supported}; \
         this desk does not store it, because a show or Programmer holding it would be refused \
         when it is opened again. Nothing was changed. {instead}.",
        family.label()
    )
}

/// Write gate for authored programming objects (TL-552 follow-up). At `supported` ≥ 1 a write
/// whose body holds legacy programming is refused before anything is stored, so a save at
/// contract 1 can never produce a show that the load-time validator rejects. Non-programming
/// kinds are never inspected. Always accepts at 0.
pub fn check_programming_object_writes<'a>(
    supported: u16,
    writes: impl IntoIterator<Item = (&'a str, &'a str, &'a Value)>,
) -> Result<(), ProgrammingContractRejection> {
    if supported == 0 {
        return Ok(());
    }
    let mut legacy = Vec::new();
    for (kind, id, body) in writes {
        if PROGRAMMING_OBJECT_KINDS.contains(&kind) {
            push_findings(&mut legacy, kind, id, body);
        }
    }
    if legacy.is_empty() {
        return Ok(());
    }
    let report = ShowProgrammingContractReport {
        show_name: String::new(),
        marker: ProgrammingContractMarker::Absent,
        legacy,
    };
    Err(ProgrammingContractRejection {
        message: format!(
            "This write holds programming from before semantic programming contract {supported} \
             ({}). It was refused and nothing was changed, because the show would be refused \
             when it is opened again. Re-record it with the current Color, Position and Zoom \
             controls.",
            report.summary()
        ),
    })
}

/// Direct writers' gate: refuses a legacy programming body at `writer_contract` ≥ 1 before the
/// SQLite transaction writes anything.
pub(crate) fn check_direct_writes<'a>(
    writer_contract: u16,
    writes: impl IntoIterator<Item = (&'a str, &'a str, &'a Value)>,
) -> Result<(), StoreError> {
    check_programming_object_writes(writer_contract, writes)
        .map_err(|rejection| StoreError::Invalid(rejection.message))
}

/// One row addressed by `"attribute"`: a Cue change or Programmer value carries its value next to
/// it, a legacy scalar Dynamic lane carries its `mode` instead (percentage-domain by
/// construction). Position and Color component rows are legacy whatever they carry. A `zoom`
/// row is legacy only when it is a percentage: a `normalized`/`spread` value, a Programmer FixAT
/// percentage (`type: fix_at`) or static percentage, or a scalar lane. A Release of the Zoom
/// owner (a Cue release row, a Group release, a Programmer `release`) and a running typed Zoom
/// Dynamic share the address and are not legacy (TL-552 follow-up: they were false positives
/// that a contract-1 desk could itself write).
fn legacy_row(attribute: &str, row: &serde_json::Map<String, Value>) -> bool {
    let percentage =
        |kind: Option<&str>| kind.is_some_and(|kind| matches!(kind, "normalized" | "spread"));
    match legacy_programming_family(attribute) {
        Some(LegacyProgrammingFamily::Zoom) => match row.get("value") {
            Some(Value::Object(value)) => match value.get("kind").and_then(Value::as_str) {
                Some(kind) => percentage(Some(kind)),
                None => match value.get("type").and_then(Value::as_str) {
                    Some("fix_at") => true,
                    Some("static") => percentage(
                        value
                            .get("value")
                            .and_then(|value| value.get("kind"))
                            .and_then(Value::as_str),
                    ),
                    _ => false,
                },
            },
            _ => row.contains_key("mode"),
        },
        Some(_) => true,
        None => false,
    }
}

fn collect(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(attribute)) = map.get("attribute")
                && legacy_row(attribute, map)
            {
                found.insert(attribute.clone());
            }
            for (key, child) in map {
                if let Some(kind) = child.get("kind").and_then(Value::as_str)
                    && legacy_value_kind(key, Some(kind))
                {
                    found.insert(key.clone());
                }
                collect(child, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect(item, found)),
        _ => {}
    }
}

/// The stored marker, distinguishing an absent key from an unreadable value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingContractMarker {
    Absent,
    Declared(u16),
    Malformed(String),
}

impl ProgrammingContractMarker {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            None => Self::Absent,
            Some(value) => value
                .parse::<u16>()
                .map_or_else(|_| Self::Malformed(value.to_owned()), Self::Declared),
        }
    }
}

/// Read-only inspection of one show file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShowProgrammingContractReport {
    pub show_name: String,
    pub marker: ProgrammingContractMarker,
    pub legacy: Vec<LegacyProgrammingFinding>,
}

/// Visible, actionable reason a show cannot load at the runtime's contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingContractRejection {
    pub message: String,
}

impl fmt::Display for ProgrammingContractRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

const RECOVERY: &str = "The show file was not changed. Open or create a different show to keep \
working; this desk's settings are unaffected. To use this show, open it in a ToskLight build \
that supports programming contract 0, or re-record these Presets, Cues, Groups and Dynamics \
with the current Color, Position and Zoom controls.";

impl ShowProgrammingContractReport {
    /// Accepts or rejects the show for a runtime supporting `supported`. Always accepts at 0.
    pub fn check(&self, supported: u16) -> Result<(), ProgrammingContractRejection> {
        if supported == 0 {
            return Ok(());
        }
        let name = &self.show_name;
        match &self.marker {
            ProgrammingContractMarker::Malformed(value) => {
                return Err(ProgrammingContractRejection {
                    message: format!(
                        "Show '{name}' has an unreadable programming contract marker \
                         ({PROGRAMMING_CONTRACT_METADATA_KEY} = {value:?}). {RECOVERY}"
                    ),
                });
            }
            ProgrammingContractMarker::Declared(declared) if *declared > supported => {
                return Err(ProgrammingContractRejection {
                    message: format!(
                        "Show '{name}' was written for programming contract {declared}; this \
                         desk supports {supported}. The show file was not changed. Open it in \
                         a newer ToskLight build, or open or create a different show."
                    ),
                });
            }
            _ => {}
        }
        if self.legacy.is_empty() {
            return Ok(());
        }
        Err(ProgrammingContractRejection {
            message: format!(
                "Show '{name}' holds programming from before semantic programming contract \
                 {supported} that cannot be converted safely (a percentage is never \
                 reinterpreted as degrees or as a colour): {}. {RECOVERY}",
                self.summary()
            ),
        })
    }

    fn summary(&self) -> String {
        const SHOWN: usize = 6;
        let mut parts = self
            .legacy
            .iter()
            .take(SHOWN)
            .map(|finding| {
                format!(
                    "{} {} {} ({})",
                    finding.kind,
                    finding.id,
                    finding.attribute,
                    finding.family.label()
                )
            })
            .collect::<Vec<_>>();
        if self.legacy.len() > SHOWN {
            parts.push(format!("and {} more", self.legacy.len() - SHOWN));
        }
        parts.join(", ")
    }
}

/// Inspects a show file through a read-only connection. Nothing is migrated or written.
pub fn inspect_show_programming_contract(
    path: impl AsRef<Path>,
) -> Result<ShowProgrammingContractReport, StoreError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    inspect_connection(&conn)
}

fn inspect_connection(conn: &Connection) -> Result<ShowProgrammingContractReport, StoreError> {
    let metadata = |key: &str| -> Result<Option<String>, StoreError> {
        conn.query_row("SELECT value FROM metadata WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(Into::into)
    };
    let show_name = metadata("name")?.unwrap_or_else(|| "unnamed show".into());
    let marker =
        ProgrammingContractMarker::parse(metadata(PROGRAMMING_CONTRACT_METADATA_KEY)?.as_deref());
    let mut legacy = Vec::new();
    let mut objects = conn.prepare("SELECT kind,id,body_json FROM objects ORDER BY kind,id")?;
    let rows = objects.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (kind, id, body) = row?;
        if PROGRAMMING_OBJECT_KINDS.contains(&kind.as_str()) {
            push_findings(&mut legacy, &kind, &id, &serde_json::from_str(&body)?);
        }
    }
    inspect_cue_rows(conn, &mut legacy)?;
    Ok(ShowProgrammingContractReport {
        show_name,
        marker,
        legacy,
    })
}

/// The pre-object `cues` table (still created by the schema) holds Cue rows as JSON too.
fn inspect_cue_rows(
    conn: &Connection,
    legacy: &mut Vec<LegacyProgrammingFinding>,
) -> Result<(), StoreError> {
    let mut cues = conn.prepare(
        "SELECT cue_list_id,cue_number,values_json,COALESCE(cue_only_restore_json,'null') FROM cues",
    )?;
    let rows = cues.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (list, number, values, restore) = row?;
        let id = format!("{list}/{number}");
        for body in [values, restore] {
            push_findings(legacy, "cue", &id, &serde_json::from_str(&body)?);
        }
    }
    Ok(())
}

fn push_findings(legacy: &mut Vec<LegacyProgrammingFinding>, kind: &str, id: &str, body: &Value) {
    for attribute in legacy_programming_attributes(body) {
        let family = legacy_programming_family(&attribute).expect("collected legacy address");
        legacy.push(LegacyProgrammingFinding {
            kind: kind.into(),
            id: id.into(),
            attribute,
            family,
        });
    }
}

/// Load/activation gate. Dormant at contract 0: returns without opening the file.
pub fn validate_show_programming_contract(
    path: impl AsRef<Path>,
    supported: u16,
) -> Result<(), StoreError> {
    if supported == 0 {
        return Ok(());
    }
    inspect_show_programming_contract(path)?
        .check(supported)
        .map_err(|rejection| StoreError::Invalid(rejection.message))
}

/// Whether a writer at `supported` stamps the marker on a transaction changing these kinds.
pub fn writer_stamps_programming_contract<'a>(
    supported: u16,
    mut changed_kinds: impl Iterator<Item = &'a str>,
) -> bool {
    supported >= 1 && changed_kinds.any(|kind| PROGRAMMING_OBJECT_KINDS.contains(&kind))
}

/// Direct (non-transaction-object) writers: stamps the marker inside the caller's SQLite
/// transaction when `writer_contract` ≥ 1 and the write touches authored programming (TL-552).
pub(crate) fn stamp_direct_write<'a>(
    tx: &rusqlite::Transaction<'_>,
    writer_contract: u16,
    changed_kinds: impl Iterator<Item = &'a str>,
) -> Result<(), StoreError> {
    if writer_stamps_programming_contract(writer_contract, changed_kinds) {
        tx.execute(
            "INSERT INTO metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![PROGRAMMING_CONTRACT_METADATA_KEY, writer_contract.to_string()],
        )?;
    }
    Ok(())
}

impl crate::PortableShowTransaction {
    /// TL-552 follow-up: refuses this transaction at `supported` ≥ 1 when one of its object
    /// writes (including an Undo/Redo body) holds legacy programming.
    pub fn check_programming_contract(
        &self,
        supported: u16,
    ) -> Result<(), ProgrammingContractRejection> {
        check_programming_object_writes(
            supported,
            self.object_writes()
                .map(|(key, body)| (key.kind(), key.id(), body)),
        )
    }

    /// Stamps the programming-contract marker into this transaction when the writer runs at
    /// contract ≥ 1 and the transaction writes or deletes authored programming. Returns whether
    /// it stamped. At contract 0 it never stamps, so production files are unchanged.
    pub fn stamp_programming_contract(&mut self, supported: u16) -> bool {
        let stamp = writer_stamps_programming_contract(supported, self.changed_object_kinds());
        if stamp {
            self.set_metadata(PROGRAMMING_CONTRACT_METADATA_KEY, supported.to_string());
        }
        stamp
    }
}

impl crate::PortableShowDocument {
    pub fn programming_contract_marker(&self) -> ProgrammingContractMarker {
        ProgrammingContractMarker::parse(
            self.metadata()
                .get(PROGRAMMING_CONTRACT_METADATA_KEY)
                .map(String::as_str),
        )
    }
}

impl crate::ShowStore {
    pub fn programming_contract_marker(&self) -> Result<ProgrammingContractMarker, StoreError> {
        Ok(ProgrammingContractMarker::parse(
            self.metadata_value(PROGRAMMING_CONTRACT_METADATA_KEY)?
                .as_deref(),
        ))
    }

    /// Inspects this open store (used by tests and by writers that already hold the store).
    pub fn programming_contract_report(&self) -> Result<ShowProgrammingContractReport, StoreError> {
        inspect_connection(&self.conn)
    }
}
