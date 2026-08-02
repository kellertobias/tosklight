use super::FixtureLibrary;
use crate::{FixtureError, GelDefinitionSnapshot};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_CATALOG_NAME_BYTES: usize = 256;

/// One installation-owned gel catalog.
///
/// Only an assigned entry's [`GelDefinitionSnapshot`] crosses into a portable show. Catalogs and
/// their revisions remain desk-local library data.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GelCatalog {
    pub id: Uuid,
    pub revision: u32,
    pub name: String,
    pub entries: Vec<GelCatalogEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GelCatalogEntry {
    pub id: Uuid,
    pub number: String,
    pub name: String,
    pub display_srgb: String,
    pub visualizer_srgb: String,
}

impl GelCatalogEntry {
    pub fn portable_snapshot(&self) -> GelDefinitionSnapshot {
        GelDefinitionSnapshot {
            number: self.number.clone(),
            name: self.name.clone(),
            display_srgb: self.display_srgb.clone(),
            visualizer_srgb: self.visualizer_srgb.clone(),
        }
    }
}

/// Whether an import creates a new stable catalog or merges rows into one known revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GelCatalogImportTarget {
    Create {
        catalog_id: Uuid,
    },
    Update {
        catalog_id: Uuid,
        expected_revision: u32,
    },
}

impl GelCatalogImportTarget {
    fn catalog_id(self) -> Uuid {
        match self {
            Self::Create { catalog_id } | Self::Update { catalog_id, .. } => catalog_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GelCatalogImportAddition {
    pub entry: GelCatalogEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GelCatalogImportReplacement {
    pub previous: GelCatalogEntry,
    pub replacement: GelCatalogEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GelCatalogImportUnchanged {
    pub entry: GelCatalogEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GelCatalogImportConflict {
    CatalogIdentityAlreadyExists {
        catalog_id: Uuid,
    },
    CatalogMissing {
        catalog_id: Uuid,
    },
    RevisionMismatch {
        catalog_id: Uuid,
        expected: u32,
        current: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GelCatalogCsvError {
    /// One-based physical CSV row. Encoding/header failures are reported against row one.
    pub row: usize,
    pub message: String,
}

/// Immutable result of parsing and comparing one CSV import.
///
/// The normalized candidate operations are intentionally private. Confirmation can therefore use
/// exactly the rows that were previewed rather than trusting a caller-modified summary.
#[derive(Clone, Debug)]
pub struct GelCatalogImportPreview {
    target: GelCatalogImportTarget,
    catalog_name: String,
    catalog_name_changed: bool,
    additions: Vec<GelCatalogImportAddition>,
    replacements: Vec<GelCatalogImportReplacement>,
    unchanged: Vec<GelCatalogImportUnchanged>,
    conflicts: Vec<GelCatalogImportConflict>,
    invalid_rows: Vec<GelCatalogCsvError>,
    operations: Vec<ImportOperation>,
}

impl GelCatalogImportPreview {
    pub fn catalog_id(&self) -> Uuid {
        self.target.catalog_id()
    }

    pub fn catalog_name(&self) -> &str {
        &self.catalog_name
    }

    pub fn catalog_name_changed(&self) -> bool {
        self.catalog_name_changed
    }

    pub fn additions(&self) -> &[GelCatalogImportAddition] {
        &self.additions
    }

    pub fn replacements(&self) -> &[GelCatalogImportReplacement] {
        &self.replacements
    }

    pub fn unchanged(&self) -> &[GelCatalogImportUnchanged] {
        &self.unchanged
    }

    pub fn conflicts(&self) -> &[GelCatalogImportConflict] {
        &self.conflicts
    }

    pub fn invalid_rows(&self) -> &[GelCatalogCsvError] {
        &self.invalid_rows
    }

    pub fn is_confirmable(&self) -> bool {
        self.conflicts.is_empty() && self.invalid_rows.is_empty()
    }
}

#[derive(Clone, Debug)]
enum ImportOperation {
    Addition {
        entry: GelCatalogEntry,
        sort_order: i64,
    },
    Replacement(GelCatalogEntry),
    Unchanged,
}

impl FixtureLibrary {
    pub fn gel_catalogs(&self) -> Result<Vec<GelCatalog>, FixtureError> {
        let ids = {
            let mut statement = self
                .conn
                .prepare("SELECT id FROM gel_catalogs ORDER BY name COLLATE NOCASE,id")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        ids.into_iter()
            .map(|id| {
                let id = parse_stored_uuid(&id, "gel catalog")?;
                self.gel_catalog(id)?.ok_or_else(|| {
                    FixtureError::Invalid(format!("gel catalog {id} disappeared while listing"))
                })
            })
            .collect()
    }

    pub fn gel_catalog(&self, id: Uuid) -> Result<Option<GelCatalog>, FixtureError> {
        let header = self
            .conn
            .query_row(
                "SELECT revision,name FROM gel_catalogs WHERE id=?1",
                [id.to_string()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((revision, name)) = header else {
            return Ok(None);
        };
        let mut statement = self.conn.prepare(
            "SELECT entry_id,number,name,display_srgb,visualizer_srgb FROM gel_catalog_entries WHERE catalog_id=?1 ORDER BY sort_order,entry_id",
        )?;
        let rows = statement.query_map([id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (entry_id, number, name, display_srgb, visualizer_srgb) = row?;
            entries.push(GelCatalogEntry {
                id: parse_stored_uuid(&entry_id, "gel catalog entry")?,
                number,
                name,
                display_srgb,
                visualizer_srgb,
            });
        }
        Ok(Some(GelCatalog {
            id,
            revision,
            name,
            entries,
        }))
    }

    /// Parses and compares a strict four-column UTF-8 CSV without changing installation data.
    ///
    /// Updating a catalog is an additive merge: matching numbers retain their stable entry ID and
    /// are replaced, new numbers are appended, and entries absent from the CSV remain installed.
    pub fn preview_gel_catalog_csv_import(
        &self,
        target: GelCatalogImportTarget,
        catalog_name: &str,
        csv: &[u8],
    ) -> Result<GelCatalogImportPreview, FixtureError> {
        validate_catalog_name(catalog_name)?;
        if target.catalog_id().is_nil() {
            return Err(FixtureError::Invalid(
                "gel catalog identity must not be nil".into(),
            ));
        }

        let existing = self.gel_catalog(target.catalog_id())?;
        let mut conflicts = Vec::new();
        match (target, existing.as_ref()) {
            (GelCatalogImportTarget::Create { catalog_id }, Some(_)) => {
                conflicts
                    .push(GelCatalogImportConflict::CatalogIdentityAlreadyExists { catalog_id });
            }
            (GelCatalogImportTarget::Update { catalog_id, .. }, None) => {
                conflicts.push(GelCatalogImportConflict::CatalogMissing { catalog_id });
            }
            (
                GelCatalogImportTarget::Update {
                    catalog_id,
                    expected_revision,
                },
                Some(current),
            ) if current.revision != expected_revision => {
                conflicts.push(GelCatalogImportConflict::RevisionMismatch {
                    catalog_id,
                    expected: expected_revision,
                    current: current.revision,
                });
            }
            _ => {}
        }

        let ParsedCatalogCsv {
            rows,
            mut invalid_rows,
        } = parse_catalog_csv(csv);
        let existing_by_number = existing
            .as_ref()
            .map(|catalog| {
                catalog
                    .entries
                    .iter()
                    .cloned()
                    .map(|entry| (entry.number.clone(), entry))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let next_sort_order = if existing.is_some() {
            self.conn.query_row(
                "SELECT COALESCE(MAX(sort_order),-1)+1 FROM gel_catalog_entries WHERE catalog_id=?1",
                [target.catalog_id().to_string()],
                |row| row.get::<_, i64>(0),
            )?
        } else {
            0
        };
        let mut additions = Vec::new();
        let mut replacements = Vec::new();
        let mut unchanged = Vec::new();
        let mut operations = Vec::new();
        let mut addition_offset = 0_i64;

        for row in rows {
            let entry = if let Some(previous) = existing_by_number.get(&row.snapshot.number) {
                GelCatalogEntry {
                    id: previous.id,
                    number: row.snapshot.number,
                    name: row.snapshot.name,
                    display_srgb: row.snapshot.display_srgb,
                    visualizer_srgb: row.snapshot.visualizer_srgb,
                }
            } else {
                GelCatalogEntry {
                    id: Uuid::new_v4(),
                    number: row.snapshot.number,
                    name: row.snapshot.name,
                    display_srgb: row.snapshot.display_srgb,
                    visualizer_srgb: row.snapshot.visualizer_srgb,
                }
            };
            if let Some(previous) = existing_by_number.get(&entry.number) {
                if previous == &entry {
                    unchanged.push(GelCatalogImportUnchanged {
                        entry: entry.clone(),
                    });
                    operations.push(ImportOperation::Unchanged);
                } else {
                    replacements.push(GelCatalogImportReplacement {
                        previous: previous.clone(),
                        replacement: entry.clone(),
                    });
                    operations.push(ImportOperation::Replacement(entry));
                }
            } else {
                additions.push(GelCatalogImportAddition {
                    entry: entry.clone(),
                });
                operations.push(ImportOperation::Addition {
                    entry,
                    sort_order: next_sort_order + addition_offset,
                });
                addition_offset += 1;
            }
        }

        if additions.is_empty()
            && replacements.is_empty()
            && unchanged.is_empty()
            && invalid_rows.is_empty()
        {
            invalid_rows.push(GelCatalogCsvError {
                row: 2,
                message: "gel catalog CSV must contain at least one data row".into(),
            });
        }

        Ok(GelCatalogImportPreview {
            target,
            catalog_name: catalog_name.to_owned(),
            catalog_name_changed: existing
                .as_ref()
                .is_none_or(|catalog| catalog.name != catalog_name),
            additions,
            replacements,
            unchanged,
            conflicts,
            invalid_rows,
            operations,
        })
    }

    /// Commits exactly one previously previewed import in one SQLite transaction.
    pub fn confirm_gel_catalog_csv_import(
        &self,
        preview: &GelCatalogImportPreview,
    ) -> Result<GelCatalog, FixtureError> {
        if !preview.is_confirmable() {
            return Err(FixtureError::Invalid(
                "gel catalog import has conflicts or invalid rows and cannot be confirmed".into(),
            ));
        }
        let transaction = self.conn.unchecked_transaction()?;
        let current_revision = transaction
            .query_row(
                "SELECT revision FROM gel_catalogs WHERE id=?1",
                [preview.catalog_id().to_string()],
                |row| row.get::<_, u32>(0),
            )
            .optional()?;

        let revision = match preview.target {
            GelCatalogImportTarget::Create { catalog_id } => {
                if current_revision.is_some() {
                    return Err(stale_preview(format!(
                        "gel catalog {catalog_id} was created after preview"
                    )));
                }
                transaction.execute(
                    "INSERT INTO gel_catalogs(id,name,revision) VALUES(?1,?2,1)",
                    params![catalog_id.to_string(), preview.catalog_name],
                )?;
                1
            }
            GelCatalogImportTarget::Update {
                catalog_id,
                expected_revision,
            } => {
                let Some(current_revision) = current_revision else {
                    return Err(stale_preview(format!(
                        "gel catalog {catalog_id} was removed after preview"
                    )));
                };
                if current_revision != expected_revision {
                    return Err(stale_preview(format!(
                        "gel catalog {catalog_id} changed from revision {expected_revision} to {current_revision} after preview"
                    )));
                }
                let has_changes = preview.catalog_name_changed
                    || preview
                        .operations
                        .iter()
                        .any(|operation| !matches!(operation, ImportOperation::Unchanged));
                if !has_changes {
                    transaction.commit()?;
                    return self.gel_catalog(catalog_id)?.ok_or_else(|| {
                        FixtureError::Invalid(format!(
                            "gel catalog {catalog_id} disappeared after an unchanged import"
                        ))
                    });
                }
                let revision = current_revision.checked_add(1).ok_or_else(|| {
                    FixtureError::Invalid("gel catalog revision is exhausted".into())
                })?;
                transaction.execute(
                    "UPDATE gel_catalogs SET name=?1,revision=?2 WHERE id=?3 AND revision=?4",
                    params![
                        preview.catalog_name,
                        revision,
                        catalog_id.to_string(),
                        current_revision
                    ],
                )?;
                revision
            }
        };

        for operation in &preview.operations {
            match operation {
                ImportOperation::Addition { entry, sort_order } => {
                    transaction.execute(
                        "INSERT INTO gel_catalog_entries(catalog_id,entry_id,number,name,display_srgb,visualizer_srgb,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                        params![preview.catalog_id().to_string(), entry.id.to_string(), entry.number, entry.name, entry.display_srgb, entry.visualizer_srgb, sort_order],
                    )?;
                }
                ImportOperation::Replacement(entry) => {
                    let updated = transaction.execute(
                        "UPDATE gel_catalog_entries SET name=?1,display_srgb=?2,visualizer_srgb=?3 WHERE catalog_id=?4 AND entry_id=?5 AND number=?6",
                        params![entry.name, entry.display_srgb, entry.visualizer_srgb, preview.catalog_id().to_string(), entry.id.to_string(), entry.number],
                    )?;
                    if updated != 1 {
                        return Err(stale_preview(format!(
                            "gel catalog entry {} changed after preview",
                            entry.number
                        )));
                    }
                }
                ImportOperation::Unchanged => {}
            }
        }
        transaction.commit()?;
        self.gel_catalog(preview.catalog_id())?.ok_or_else(|| {
            FixtureError::Invalid(format!(
                "gel catalog {} disappeared after import revision {revision}",
                preview.catalog_id()
            ))
        })
    }
}

fn validate_catalog_name(name: &str) -> Result<(), FixtureError> {
    if name.is_empty() || name.trim() != name || name.len() > MAX_CATALOG_NAME_BYTES {
        return Err(FixtureError::Invalid(format!(
            "gel catalog name must be trimmed and contain 1-{MAX_CATALOG_NAME_BYTES} bytes"
        )));
    }
    Ok(())
}

fn parse_stored_uuid(value: &str, label: &str) -> Result<Uuid, FixtureError> {
    Uuid::parse_str(value)
        .map_err(|_| FixtureError::Invalid(format!("stored {label} identity is invalid: {value}")))
}

fn stale_preview(message: String) -> FixtureError {
    FixtureError::Invalid(format!("stale gel catalog import preview: {message}"))
}

struct ParsedCatalogCsv {
    rows: Vec<ParsedCatalogRow>,
    invalid_rows: Vec<GelCatalogCsvError>,
}

struct ParsedCatalogRow {
    snapshot: GelDefinitionSnapshot,
}

fn parse_catalog_csv(bytes: &[u8]) -> ParsedCatalogCsv {
    let source = match std::str::from_utf8(bytes) {
        Ok(source) => source.strip_prefix('\u{feff}').unwrap_or(source),
        Err(error) => {
            return ParsedCatalogCsv {
                rows: Vec::new(),
                invalid_rows: vec![GelCatalogCsvError {
                    row: 1,
                    message: format!(
                        "gel catalog CSV must be readable UTF-8 (invalid byte at offset {})",
                        error.valid_up_to()
                    ),
                }],
            };
        }
    };
    let records = parse_csv_records(source);
    let mut invalid_rows = Vec::new();
    let Some(header) = records.first() else {
        return ParsedCatalogCsv {
            rows: Vec::new(),
            invalid_rows: vec![GelCatalogCsvError {
                row: 1,
                message: "gel catalog CSV is missing its header row".into(),
            }],
        };
    };
    if let Some(message) = &header.error {
        invalid_rows.push(GelCatalogCsvError {
            row: header.row,
            message: message.clone(),
        });
    }
    let expected_header = ["number", "name", "display_rgb", "visualizer_rgb"];
    let header_valid = header.error.is_none()
        && header.fields.len() == expected_header.len()
        && header.fields.iter().map(String::as_str).eq(expected_header);
    if !header_valid && header.error.is_none() {
        invalid_rows.push(GelCatalogCsvError {
            row: header.row,
            message:
                "gel catalog CSV header must be exactly number,name,display_rgb,visualizer_rgb"
                    .into(),
        });
    }
    if !header_valid {
        return ParsedCatalogCsv {
            rows: Vec::new(),
            invalid_rows,
        };
    }

    let mut rows = Vec::new();
    let mut numbers = HashSet::new();
    for record in records.into_iter().skip(1) {
        if let Some(message) = record.error {
            invalid_rows.push(GelCatalogCsvError {
                row: record.row,
                message,
            });
            continue;
        }
        if record.fields.len() != 4 {
            invalid_rows.push(GelCatalogCsvError {
                row: record.row,
                message: format!(
                    "gel catalog CSV row must contain exactly 4 columns, found {}",
                    record.fields.len()
                ),
            });
            continue;
        }
        let snapshot = GelDefinitionSnapshot {
            number: record.fields[0].clone(),
            name: record.fields[1].clone(),
            display_srgb: record.fields[2].clone(),
            visualizer_srgb: record.fields[3].clone(),
        };
        if let Err(message) = validate_snapshot(&snapshot) {
            invalid_rows.push(GelCatalogCsvError {
                row: record.row,
                message,
            });
            continue;
        }
        if !numbers.insert(snapshot.number.clone()) {
            invalid_rows.push(GelCatalogCsvError {
                row: record.row,
                message: format!(
                    "gel catalog number {} is duplicated within this CSV",
                    snapshot.number
                ),
            });
            continue;
        }
        rows.push(ParsedCatalogRow { snapshot });
    }
    ParsedCatalogCsv { rows, invalid_rows }
}

fn validate_snapshot(snapshot: &GelDefinitionSnapshot) -> Result<(), String> {
    validate_trimmed(&snapshot.number, "gel catalog number", 128)?;
    validate_trimmed(&snapshot.name, "gel display name", 256)?;
    validate_srgb(&snapshot.display_srgb, "gel display color")?;
    validate_srgb(&snapshot.visualizer_srgb, "gel visualizer color")
}

fn validate_trimmed(value: &str, label: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.trim() != value || value.len() > max_bytes {
        return Err(format!(
            "{label} must be trimmed and contain 1-{max_bytes} bytes"
        ));
    }
    Ok(())
}

fn validate_srgb(value: &str, label: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 7
        || bytes[0] != b'#'
        || bytes[1..]
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'A'..=b'F').contains(byte))
    {
        return Err(format!("{label} must be canonical #RRGGBB sRGB"));
    }
    Ok(())
}

struct CsvRecord {
    row: usize,
    fields: Vec<String>,
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum CsvFieldState {
    Start,
    Unquoted,
    Quoted,
    QuoteClosed,
}

fn parse_csv_records(source: &str) -> Vec<CsvRecord> {
    CsvParser::new(source).parse()
}

struct CsvParser<'a> {
    bytes: &'a [u8],
    records: Vec<CsvRecord>,
    fields: Vec<String>,
    field: Vec<u8>,
    state: CsvFieldState,
    row: usize,
    record_row: usize,
    record_started: bool,
    error: Option<String>,
    index: usize,
}

impl<'a> CsvParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            bytes: source.as_bytes(),
            records: Vec::new(),
            fields: Vec::new(),
            field: Vec::new(),
            state: CsvFieldState::Start,
            row: 1,
            record_row: 1,
            record_started: false,
            error: None,
            index: 0,
        }
    }

    fn parse(mut self) -> Vec<CsvRecord> {
        while self.index < self.bytes.len() {
            let byte = self.bytes[self.index];
            match self.state {
                CsvFieldState::Start => self.consume_start(byte),
                CsvFieldState::Unquoted => self.consume_unquoted(byte),
                CsvFieldState::Quoted => self.consume_quoted(byte),
                CsvFieldState::QuoteClosed => self.consume_quote_closed(byte),
            }
            self.index += 1;
        }
        if matches!(self.state, CsvFieldState::Quoted) {
            self.error
                .get_or_insert_with(|| "unterminated quoted CSV field".into());
        }
        if self.record_started || !self.fields.is_empty() || !self.field.is_empty() {
            self.finish_record();
        }
        self.records
    }

    fn consume_start(&mut self, byte: u8) {
        match byte {
            b',' => {
                self.record_started = true;
                self.fields.push(String::new());
            }
            b'"' => {
                self.record_started = true;
                self.state = CsvFieldState::Quoted;
            }
            b'\n' => self.finish_line(false),
            b'\r' if self.next_is_newline() => self.finish_line(true),
            b'\r' => {
                self.carriage_return_error();
                self.finish_line(false);
            }
            _ => {
                self.record_started = true;
                self.field.push(byte);
                self.state = CsvFieldState::Unquoted;
            }
        }
    }

    fn consume_unquoted(&mut self, byte: u8) {
        match byte {
            b',' => {
                push_csv_field(&mut self.fields, &mut self.field);
                self.state = CsvFieldState::Start;
            }
            b'"' => {
                self.error
                    .get_or_insert_with(|| "unescaped quote in an unquoted CSV field".into());
                self.field.push(byte);
            }
            b'\n' => self.finish_line(false),
            b'\r' if self.next_is_newline() => self.finish_line(true),
            b'\r' => {
                self.carriage_return_error();
                self.finish_line(false);
            }
            _ => self.field.push(byte),
        }
    }

    fn consume_quoted(&mut self, byte: u8) {
        match byte {
            b'"' if self.bytes.get(self.index + 1) == Some(&b'"') => {
                self.field.push(b'"');
                self.index += 1;
            }
            b'"' => self.state = CsvFieldState::QuoteClosed,
            b'\n' => {
                self.field.push(byte);
                self.row += 1;
            }
            b'\r' if self.next_is_newline() => {
                self.field.extend_from_slice(b"\r\n");
                self.index += 1;
                self.row += 1;
            }
            _ => self.field.push(byte),
        }
    }

    fn consume_quote_closed(&mut self, byte: u8) {
        match byte {
            b',' => {
                push_csv_field(&mut self.fields, &mut self.field);
                self.state = CsvFieldState::Start;
            }
            b'\n' => self.finish_line(false),
            b'\r' if self.next_is_newline() => self.finish_line(true),
            _ => {
                self.error.get_or_insert_with(|| {
                    "unexpected character after a closing quote in CSV field".into()
                });
                self.field.push(byte);
                self.state = CsvFieldState::Unquoted;
            }
        }
    }

    fn finish_line(&mut self, consume_newline: bool) {
        self.finish_record();
        self.state = CsvFieldState::Start;
        if consume_newline {
            self.index += 1;
        }
        self.row += 1;
        self.record_row = self.row;
        self.record_started = false;
    }

    fn finish_record(&mut self) {
        finish_csv_record(
            &mut self.records,
            &mut self.fields,
            &mut self.field,
            self.record_row,
            &mut self.error,
        );
    }

    fn next_is_newline(&self) -> bool {
        self.bytes.get(self.index + 1) == Some(&b'\n')
    }

    fn carriage_return_error(&mut self) {
        self.error
            .get_or_insert_with(|| "CSV uses a carriage return without a following newline".into());
    }
}

fn push_csv_field(fields: &mut Vec<String>, field: &mut Vec<u8>) {
    fields.push(String::from_utf8(std::mem::take(field)).expect("validated UTF-8 CSV field"));
}

fn finish_csv_record(
    records: &mut Vec<CsvRecord>,
    fields: &mut Vec<String>,
    field: &mut Vec<u8>,
    row: usize,
    error: &mut Option<String>,
) {
    push_csv_field(fields, field);
    records.push(CsvRecord {
        row,
        fields: std::mem::take(fields),
        error: error.take(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "number,name,display_rgb,visualizer_rgb\n";

    fn library() -> FixtureLibrary {
        FixtureLibrary::open(":memory:").unwrap()
    }

    fn create_preview(
        library: &FixtureLibrary,
        catalog_id: Uuid,
        rows: &str,
    ) -> GelCatalogImportPreview {
        library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Create { catalog_id },
                "House gels",
                format!("{HEADER}{rows}").as_bytes(),
            )
            .unwrap()
    }

    #[test]
    fn utf8_quoted_csv_previews_then_confirms_normalized_installation_data() {
        let library = library();
        let catalog_id = Uuid::new_v4();
        let preview = create_preview(
            &library,
            catalog_id,
            "A1,Amber,#FFAA00,#F08000\n\"B,2\",\"Blå, cool\",#0088FF,#0066EE\n",
        );
        assert!(preview.is_confirmable());
        assert_eq!(preview.additions().len(), 2);
        assert_eq!(preview.additions()[1].entry.number, "B,2");
        assert_eq!(preview.additions()[1].entry.name, "Blå, cool");
        assert!(library.gel_catalog(catalog_id).unwrap().is_none());

        let catalog = library.confirm_gel_catalog_csv_import(&preview).unwrap();
        assert_eq!(catalog.revision, 1);
        assert_eq!(
            catalog.entries,
            preview
                .additions()
                .iter()
                .map(|item| item.entry.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            catalog.entries[1].portable_snapshot(),
            GelDefinitionSnapshot {
                number: "B,2".into(),
                name: "Blå, cool".into(),
                display_srgb: "#0088FF".into(),
                visualizer_srgb: "#0066EE".into(),
            }
        );
        let stored_columns = library
            .conn
            .prepare("PRAGMA table_info(gel_catalogs)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!stored_columns.iter().any(|column| column.contains("csv")));
    }

    #[test]
    fn import_rejects_encoding_header_columns_fields_colors_and_duplicate_numbers_by_row() {
        let library = library();
        let invalid_utf8 = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Create {
                    catalog_id: Uuid::new_v4(),
                },
                "Unreadable",
                &[0xFF, 0xFE],
            )
            .unwrap();
        assert_eq!(invalid_utf8.invalid_rows()[0].row, 1);
        assert!(invalid_utf8.invalid_rows()[0].message.contains("UTF-8"));

        let wrong_header = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Create {
                    catalog_id: Uuid::new_v4(),
                },
                "Wrong header",
                b"name,number,display_rgb,visualizer_rgb\nAmber,A1,#FFAA00,#F08000\n",
            )
            .unwrap();
        assert_eq!(wrong_header.invalid_rows()[0].row, 1);

        let catalog_id = Uuid::new_v4();
        let preview = create_preview(
            &library,
            catalog_id,
            "A1,Amber,#ffaa00,#F08000\nB2,,#00FF00,#00EE00\nC3,Cyan,#00FFFF,#00EEEE,extra\nD4,Deep blue,#0000FF,#0000DD\nD4,Duplicate,#1111FF,#1111DD\n",
        );
        assert_eq!(
            preview
                .invalid_rows()
                .iter()
                .map(|error| error.row)
                .collect::<Vec<_>>(),
            vec![2, 3, 4, 6]
        );
        assert_eq!(preview.additions().len(), 1);
        assert!(!preview.is_confirmable());
        assert!(library.confirm_gel_catalog_csv_import(&preview).is_err());
        assert!(library.gel_catalog(catalog_id).unwrap().is_none());
    }

    #[test]
    fn malformed_quoting_is_row_specific_and_never_partially_mutates() {
        let library = library();
        let catalog_id = Uuid::new_v4();
        let preview = create_preview(
            &library,
            catalog_id,
            "A1,Amber,#FFAA00,#F08000\nB2,Blue\" broken,#0000FF,#0000EE\nC3,Cyan,#00FFFF,#00EEEE\n",
        );
        assert_eq!(preview.invalid_rows().len(), 1);
        assert_eq!(preview.invalid_rows()[0].row, 3);
        assert!(
            preview.invalid_rows()[0]
                .message
                .contains("unescaped quote")
        );
        assert_eq!(preview.additions().len(), 2);
        assert!(library.confirm_gel_catalog_csv_import(&preview).is_err());
        assert!(library.gel_catalog(catalog_id).unwrap().is_none());

        let unterminated = create_preview(
            &library,
            Uuid::new_v4(),
            "A1,Amber,#FFAA00,#F08000\nB2,\"Blue,#0000FF,#0000EE",
        );
        assert_eq!(unterminated.invalid_rows()[0].row, 3);
        assert!(
            unterminated.invalid_rows()[0]
                .message
                .contains("unterminated")
        );
    }

    #[test]
    fn update_preview_reports_additions_replacements_and_unchanged_with_stable_entry_identity() {
        let library = library();
        let catalog_id = Uuid::new_v4();
        let initial = create_preview(
            &library,
            catalog_id,
            "A1,Amber,#FFAA00,#F08000\nB2,Blue,#0000FF,#0000EE\n",
        );
        let initial = library.confirm_gel_catalog_csv_import(&initial).unwrap();
        let amber_id = initial.entries[0].id;
        let blue_id = initial.entries[1].id;

        let preview = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Update {
                    catalog_id,
                    expected_revision: 1,
                },
                "Touring gels",
                format!(
                    "{HEADER}A1,Warm amber,#FFAA00,#EE7000\nB2,Blue,#0000FF,#0000EE\nC3,Cyan,#00FFFF,#00EEEE\n"
                )
                .as_bytes(),
            )
            .unwrap();
        assert!(preview.is_confirmable());
        assert!(preview.catalog_name_changed());
        assert_eq!(preview.replacements().len(), 1);
        assert_eq!(preview.replacements()[0].replacement.id, amber_id);
        assert_eq!(preview.unchanged().len(), 1);
        assert_eq!(preview.unchanged()[0].entry.id, blue_id);
        assert_eq!(preview.additions().len(), 1);
        assert_eq!(library.gel_catalog(catalog_id).unwrap().unwrap(), initial);

        let updated = library.confirm_gel_catalog_csv_import(&preview).unwrap();
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.name, "Touring gels");
        assert_eq!(updated.entries.len(), 3);
        assert_eq!(updated.entries[0].id, amber_id);
        assert_eq!(updated.entries[1].id, blue_id);
        assert_eq!(updated.entries[0].name, "Warm amber");
    }

    #[test]
    fn duplicate_identity_and_stale_revision_are_visible_conflicts() {
        let library = library();
        let catalog_id = Uuid::new_v4();
        let first = create_preview(&library, catalog_id, "A1,Amber,#FFAA00,#F08000\n");
        library.confirm_gel_catalog_csv_import(&first).unwrap();

        let duplicate = create_preview(&library, catalog_id, "B2,Blue,#0000FF,#0000EE\n");
        assert_eq!(
            duplicate.conflicts(),
            &[GelCatalogImportConflict::CatalogIdentityAlreadyExists { catalog_id }]
        );
        assert!(!duplicate.is_confirmable());

        let stale = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Update {
                    catalog_id,
                    expected_revision: 0,
                },
                "House gels",
                format!("{HEADER}B2,Blue,#0000FF,#0000EE\n").as_bytes(),
            )
            .unwrap();
        assert_eq!(
            stale.conflicts(),
            &[GelCatalogImportConflict::RevisionMismatch {
                catalog_id,
                expected: 0,
                current: 1,
            }]
        );
        assert_eq!(
            library
                .gel_catalog(catalog_id)
                .unwrap()
                .unwrap()
                .entries
                .len(),
            1
        );
    }

    #[test]
    fn confirmation_rechecks_revision_and_leaves_stale_preview_unapplied() {
        let library = library();
        let catalog_id = Uuid::new_v4();
        let first = create_preview(&library, catalog_id, "A1,Amber,#FFAA00,#F08000\n");
        library.confirm_gel_catalog_csv_import(&first).unwrap();
        let stale = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Update {
                    catalog_id,
                    expected_revision: 1,
                },
                "House gels",
                format!("{HEADER}B2,Blue,#0000FF,#0000EE\n").as_bytes(),
            )
            .unwrap();
        let concurrent = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Update {
                    catalog_id,
                    expected_revision: 1,
                },
                "House gels",
                format!("{HEADER}C3,Cyan,#00FFFF,#00EEEE\n").as_bytes(),
            )
            .unwrap();
        library.confirm_gel_catalog_csv_import(&concurrent).unwrap();

        let error = library
            .confirm_gel_catalog_csv_import(&stale)
            .unwrap_err()
            .to_string();
        assert!(error.contains("stale gel catalog import preview"));
        let catalog = library.gel_catalog(catalog_id).unwrap().unwrap();
        assert_eq!(catalog.revision, 2);
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|entry| entry.number.as_str())
                .collect::<Vec<_>>(),
            vec!["A1", "C3"]
        );
    }

    #[test]
    fn exact_header_only_and_invalid_catalog_identity_or_name_are_rejected() {
        let library = library();
        let empty = create_preview(&library, Uuid::new_v4(), "");
        assert_eq!(empty.invalid_rows()[0].row, 2);
        assert!(!empty.is_confirmable());
        assert!(
            library
                .preview_gel_catalog_csv_import(
                    GelCatalogImportTarget::Create {
                        catalog_id: Uuid::nil(),
                    },
                    "House gels",
                    format!("{HEADER}A1,Amber,#FFAA00,#F08000\n").as_bytes(),
                )
                .is_err()
        );
        assert!(
            library
                .preview_gel_catalog_csv_import(
                    GelCatalogImportTarget::Create {
                        catalog_id: Uuid::new_v4(),
                    },
                    " House gels ",
                    format!("{HEADER}A1,Amber,#FFAA00,#F08000\n").as_bytes(),
                )
                .is_err()
        );
    }
}
