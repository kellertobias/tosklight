//! One immutable MVR preview per document window. Tokens are invalidated on document replacement.
use super::*;
use std::sync::atomic::Ordering;

pub(super) struct PendingMvrImport {
    token: String,
    generation: u64,
    prepared: viz_document::PreparedMvrImport,
}

impl Session {
    pub(super) fn prepare_mvr(&self, path: &str) -> Answer<MvrPreviewDto> {
        let _lifecycle = self.document_lifecycle.lock();
        let archive = read_archive(path)?;
        let prepared = self.with(|document| {
            document
                .prepare_mvr_import(archive)
                .map_err(|e| e.to_string())
        })?;
        let token = Uuid::new_v4().to_string();
        let preview = &prepared.preview;
        let dto = MvrPreviewDto {
            token: token.clone(),
            warnings: preview.warnings.clone(),
            fixtures: preview
                .fixtures
                .iter()
                .map(|fixture| MvrPreviewFixtureDto {
                    uuid: fixture.uuid.to_string(),
                    name: fixture.name.clone(),
                    gdtf_spec: fixture.gdtf_spec.clone(),
                    gdtf_mode: fixture.gdtf_mode.clone(),
                    universe: fixture.universe,
                    address: fixture.address,
                    matched: fixture.matched,
                    conflicted: fixture.conflicted,
                })
                .collect(),
            scenery: preview.scenery,
            missing_profiles: preview.missing_profiles.clone(),
            address_conflicts: preview.address_conflicts.clone(),
        };
        *self.pending_mvr.lock() = Some(PendingMvrImport {
            token,
            prepared,
            generation: self.document_generation.load(Ordering::Relaxed),
        });
        Ok(dto)
    }

    pub(super) fn apply_mvr(
        &self,
        token: &str,
        resolutions: HashMap<Uuid, MvrImportResolution>,
    ) -> Answer<MvrImportReport> {
        let _lifecycle = self.document_lifecycle.lock();
        let mut pending = self.pending_mvr.lock();
        let preview = pending
            .as_ref()
            .filter(|preview| {
                preview.token == token
                    && preview.generation == self.document_generation.load(Ordering::Relaxed)
            })
            .ok_or("This MVR preview is no longer current. Preview the archive again.")?;
        // Keep the staged inputs if a decision fails; successful application consumes the token.
        let report = self.change(|document| {
            let outcome = document
                .import_prepared_mvr(preview.prepared.clone(), resolutions)
                .map_err(|e| e.to_string())?;
            Ok(MvrImportReport {
                imported_fixtures: outcome.imported_fixtures,
                unresolved_fixtures: outcome.unresolved_fixtures,
                warnings: outcome.warnings,
            })
        })?;
        *pending = None;
        Ok(report)
    }

    pub(super) fn cancel_mvr(&self, token: &str) {
        let mut pending = self.pending_mvr.lock();
        if pending
            .as_ref()
            .is_some_and(|preview| preview.token == token)
        {
            *pending = None;
        }
    }
}

/// A failed UI notification cannot turn a committed import into an apparent write failure.
pub(super) fn after_notification(
    mut report: MvrImportReport,
    result: Answer<()>,
) -> MvrImportReport {
    if let Err(error) = result {
        report.warnings.push(format!("Import was saved, but another window could not refresh: {error}. Reopen that window to refresh it."));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestFiles {
        dir: PathBuf,
    }
    impl TestFiles {
        fn new() -> Self {
            let base = std::env::var_os("LIGHT_TMP_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let dir = base.join(format!("mvr-preview-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }
        fn path(&self, name: &str) -> PathBuf {
            self.dir.join(name)
        }
        fn archive(&self) -> PathBuf {
            let path = self.path("source.mvr");
            let document = light_mvr::MvrDocument {
                fixtures: vec![light_mvr::MvrFixture {
                    uuid: Uuid::from_u128(1),
                    name: "Unresolved source".into(),
                    fixture_id: None,
                    gdtf_spec: "Unknown.gdtf".into(),
                    gdtf_mode: "Default".into(),
                    universe: Some(1),
                    address: Some(1),
                    matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
                    layer: None,
                    class: None,
                }],
                ..Default::default()
            };
            std::fs::write(&path, light_mvr::write(&document).unwrap()).unwrap();
            path
        }
    }
    impl Drop for TestFiles {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn mvr_token_uses_captured_archive_and_success_cannot_apply_twice() {
        let files = TestFiles::new();
        let session = Session::default();
        session
            .open_path(&files.path("show.show"), Some("Token test"))
            .unwrap();
        let path = files.archive();
        let preview = session.prepare_mvr(path.to_str().unwrap()).unwrap();
        std::fs::write(&path, b"changed file after preview").unwrap();
        assert_eq!(
            session
                .apply_mvr(&preview.token, HashMap::new())
                .unwrap()
                .unresolved_fixtures,
            1
        );
        assert!(
            session
                .apply_mvr(&preview.token, HashMap::new())
                .unwrap_err()
                .contains("no longer current")
        );
    }

    #[test]
    fn mvr_new_preview_cancel_and_document_replacement_revoke_only_current_token() {
        let files = TestFiles::new();
        let session = Session::default();
        session
            .open_path(&files.path("one.show"), Some("One"))
            .unwrap();
        let path = files.archive();
        let first = session.prepare_mvr(path.to_str().unwrap()).unwrap();
        let second = session.prepare_mvr(path.to_str().unwrap()).unwrap();
        assert!(session.apply_mvr(&first.token, HashMap::new()).is_err());
        session.cancel_mvr(&first.token);
        assert!(session.pending_mvr.lock().is_some());
        session.cancel_mvr(&second.token);
        assert!(session.pending_mvr.lock().is_none());
        let third = session.prepare_mvr(path.to_str().unwrap()).unwrap();
        session
            .open_path(&files.path("two.show"), Some("Two"))
            .unwrap();
        assert!(session.apply_mvr(&third.token, HashMap::new()).is_err());
    }

    #[test]
    fn mvr_failed_apply_keeps_preview_for_corrected_decision_and_reports_committed_notification_failure()
     {
        let files = TestFiles::new();
        let session = Session::default();
        session
            .open_path(&files.path("show.show"), Some("Retry"))
            .unwrap();
        // Export a real resolved native fixture from another planning show.
        let source = PlanningDocument::create(files.path("source.show"), "Source").unwrap();
        let mut profile = light_fixture::FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Fixture".into();
        profile.short_name = "Fixture".into();
        profile.revision = 1;
        source
            .retain_fixture_profile(serde_json::to_value(&profile).unwrap())
            .unwrap();
        let patch: light_fixture::PatchedFixturePatch = serde_json::from_value(serde_json::json!({
            "fixture_id": Uuid::new_v4(), "fixture_number":1, "name":"Test", "universe":1, "address":1,
            "layer_id":"default", "location": {"x":0,"y":0,"z":0}, "rotation":{"x":0,"y":0,"z":0},
            "split_patches":[{"split":1,"universe":1,"address":1}],
        })).unwrap();
        source
            .patch_fixtures(light_application::PatchFixturesCommand {
                show_id: source.show_id(),
                fixtures: vec![light_application::PatchFixtureCandidate {
                    profile: light_fixture::PatchedFixtureProfileReference {
                        profile_id: profile.id,
                        profile_revision: 1,
                        mode_id: profile.modes[0].id,
                    },
                    patch,
                }],
                remove_fixture_ids: vec![],
                placements: vec![],
                vector_spreads: vec![],
                fixture_updates: vec![],
            })
            .unwrap();
        let archive = source.export_mvr().unwrap();
        let path = files.path("resolved.mvr");
        std::fs::write(&path, archive.data).unwrap();
        let preview = session.prepare_mvr(path.to_str().unwrap()).unwrap();
        let uuid = Uuid::parse_str(&preview.fixtures[0].uuid).unwrap();
        assert!(
            session
                .apply_mvr(
                    &preview.token,
                    HashMap::from([(
                        uuid,
                        MvrImportResolution::Address {
                            universe: 1,
                            address: 0
                        }
                    )])
                )
                .is_err()
        );
        assert!(session.pending_mvr.lock().is_some());
        let report = session.apply_mvr(&preview.token, HashMap::new()).unwrap();
        assert_eq!(report.imported_fixtures, 1);
        let report = after_notification(report, Err("notification disconnected".into()));
        assert_eq!(report.imported_fixtures, 1);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("Import was saved"))
        );
    }
}
