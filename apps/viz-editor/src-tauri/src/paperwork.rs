//! Guard every delayed project-information edit against the identity of the open native show.
use crate::session::{DocumentSummary, Session, announce_document_change, summarize};
use serde::Deserialize;
use uuid::Uuid;
use viz_document::PaperworkMetadata;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperworkInput {
    pub lighting_designer: String,
    pub show_version: String,
    pub venue: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub project: String,
    pub show_date: String,
    /// Absent from a window that predates logos, which leaves no logo.
    #[serde(default)]
    pub company_logo: String,
}

pub(crate) fn store_paperwork(
    session: &Session,
    show_id: Uuid,
    paperwork: PaperworkInput,
) -> Result<DocumentSummary, String> {
    session.change(|document| {
        if show_id.is_nil() || document.show_id().0 != show_id {
            return Err(
                "The open show changed; reopen its project information before saving.".into(),
            );
        }
        document
            .save_paperwork_metadata(&PaperworkMetadata {
                lighting_designer: paperwork.lighting_designer,
                show_version: paperwork.show_version,
                venue: paperwork.venue,
                contact_email: paperwork.contact_email,
                contact_phone: paperwork.contact_phone,
                project: paperwork.project,
                show_date: paperwork.show_date,
                company_logo: paperwork.company_logo,
            })
            .map_err(|error| error.to_string())?;
        summarize(document)
    })
}

#[tauri::command]
pub fn save_document_paperwork(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    show_id: Uuid,
    paperwork: PaperworkInput,
) -> Result<DocumentSummary, String> {
    let summary = store_paperwork(&session, show_id, paperwork)?;
    announce_document_change(&app, &window)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_document::PlanningDocument;

    fn input(project: &str) -> PaperworkInput {
        PaperworkInput {
            project: project.into(),
            lighting_designer: "Designer".into(),
            show_version: "1".into(),
            venue: "Venue".into(),
            contact_email: "desk@example.com".into(),
            contact_phone: "123".into(),
            show_date: "2026-10-27".into(),
            company_logo: String::new(),
        }
    }

    #[test]
    fn autosave_updates_native_file_but_refuses_a_stale_show_identity() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("LIGHT_TMP_DIR").expect("canonical temporary directory"),
        )
        .join(format!("paperwork-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("first.show");
        let second = directory.join("second.show");
        let document = PlanningDocument::create(&path, "First").unwrap();
        let id = document.show_id().0;
        drop(document);
        drop(PlanningDocument::create(&second, "Second").unwrap());
        let session = Session::default();
        session.open(&path).unwrap();
        assert_eq!(
            store_paperwork(&session, id, input("Stage Detail"))
                .unwrap()
                .project,
            "Stage Detail"
        );
        session.open(&second).unwrap();
        assert!(
            store_paperwork(&session, id, input("Wrong show"))
                .unwrap_err()
                .contains("changed")
        );
        assert_ne!(session.with(summarize).unwrap().project, "Wrong show");
        session.open(&path).unwrap();
        assert_eq!(session.with(summarize).unwrap().project, "Stage Detail");
        drop(session);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
