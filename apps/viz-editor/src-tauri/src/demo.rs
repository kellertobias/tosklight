//! Opening the packaged demo show.
//!
//! The demo that ships is a template and stays one. Opening it never opens the packaged file: it
//! writes a fresh, ordinary copy into the operator's own shows folder and opens that, so the
//! demo can be patched, saved and thrown away as many times as anyone likes and the next operator
//! still gets the rig the release shipped. Opening it again makes another copy rather than
//! reopening the last one, because the last one may be nothing like the demo by now.

use crate::discovery::Discovery;
use crate::session::{DocumentSummary, Session};
use std::path::{Path, PathBuf};

/// The name the copies are derived from, and the base of their file names.
const DEMO_NAME: &str = "Demo Show";
const DEMO_FILE_STEM: &str = "demo-show";

/// How many copies to look past before giving up. An operator with this many demo copies has a
/// different problem, and an unbounded loop over a directory is not the way to find it out.
const MAX_COPIES: u32 = 999;

/// Where the packaged template is, for this installation or this development tree.
///
/// A release packages it beside the application's other resources. A source checkout has no
/// bundle, so the generated artefact is used directly — the same file `npm run demo-show` writes.
pub fn template_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(named) = std::env::var_os(TEMPLATE_PATH_ENV).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(named);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{TEMPLATE_PATH_ENV} names {}, which is not a file",
                path.display()
            ))
        };
    }
    let packaged = tauri::Manager::path(app)
        .resource_dir()
        .ok()
        .map(|dir| dir.join("demo-show").join("demo-show.show"))
        .filter(|path| path.is_file());
    packaged.ok_or_else(|| {
        "this build has no packaged demo show; generate one with `npm run demo-show`".to_owned()
    })
}

/// Names the demo template, for a development tree or an unusual installation.
pub const TEMPLATE_PATH_ENV: &str = "TOSKLIGHT_VIZ_DEMO_SHOW";

/// Opens a fresh writable copy of the packaged demo.
#[tauri::command]
pub fn open_demo_show(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
) -> Result<DocumentSummary, String> {
    let template = template_path(&app)?;
    let shows = tauri::Manager::path(&app)
        .app_data_dir()
        .map_err(|error| format!("this installation has no application data folder: {error}"))?
        .join("shows");
    std::fs::create_dir_all(&shows)
        .map_err(|error| format!("could not create {}: {error}", shows.display()))?;

    let summary = open_copy(&session, &template, &shows)?;
    discovery.announce_document(Some(summary.name.clone()));
    Ok(summary)
}

/// Copies `template` into `shows` and opens the copy.
///
/// Separate from the command so the interesting part — that a copy is what opens, that it is named
/// after the demo, and that the template is never touched — can be tested without a window.
fn open_copy(session: &Session, template: &Path, shows: &Path) -> Result<DocumentSummary, String> {
    let (path, name) = next_copy(shows)?;
    std::fs::copy(template, &path).map_err(|error| {
        format!(
            "could not copy the demo show to {}: {error}",
            path.display()
        )
    })?;

    let summary = session.open(&path).inspect_err(|_| {
        // A copy that will not open is this command's litter, not the operator's document.
        let _ = std::fs::remove_file(&path);
    })?;
    // The copy says what it is: an operator who opens the demo twice has to be able to tell the two
    // apart, and neither of them is the packaged template.
    session.rename_to(&name)?;
    Ok(DocumentSummary { name, ..summary })
}

/// The first demo copy name that is not taken, and the file to write it to.
///
/// Both are checked: a name is only free when no file claims it, so the numbering an operator sees
/// matches the files on disk rather than drifting from them.
fn next_copy(shows: &Path) -> Result<(PathBuf, String), String> {
    for index in 1..=MAX_COPIES {
        let (file_name, name) = if index == 1 {
            (format!("{DEMO_FILE_STEM}.show"), DEMO_NAME.to_owned())
        } else {
            (
                format!("{DEMO_FILE_STEM}-{index}.show"),
                format!("{DEMO_NAME} {index}"),
            )
        };
        let path = shows.join(file_name);
        if !path.exists() {
            return Ok((path, name));
        }
    }
    Err(format!(
        "there are already {MAX_COPIES} demo shows in {}; delete some before opening another",
        shows.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(name: &str) -> PathBuf {
        let root = std::env::var_os("LIGHT_TMP_DIR").map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp"),
            PathBuf::from,
        );
        let directory = root.join("viz-editor-demo-tests").join(name);
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("test workspace");
        directory
    }

    #[test]
    fn the_first_copy_is_named_after_the_demo_itself() {
        let shows = workspace("first");
        let (path, name) = next_copy(&shows).expect("a free name");
        assert_eq!(name, "Demo Show");
        assert_eq!(path.file_name().unwrap(), "demo-show.show");
    }

    #[test]
    fn a_second_copy_is_numbered_rather_than_overwriting_the_first() {
        let shows = workspace("second");
        std::fs::write(shows.join("demo-show.show"), b"first").expect("first copy");
        let (path, name) = next_copy(&shows).expect("a free name");
        assert_eq!(name, "Demo Show 2");
        assert_eq!(path.file_name().unwrap(), "demo-show-2.show");
        assert!(!path.exists(), "the second copy has not been written yet");
    }

    #[test]
    fn a_gap_left_by_a_deleted_copy_is_reused() {
        let shows = workspace("gap");
        std::fs::write(shows.join("demo-show.show"), b"first").expect("first copy");
        std::fs::write(shows.join("demo-show-3.show"), b"third").expect("third copy");
        let (_path, name) = next_copy(&shows).expect("a free name");
        assert_eq!(name, "Demo Show 2");
    }

    /// The real packaged demo, generated exactly as a release generates it.
    fn template(directory: &Path) -> PathBuf {
        let library = light_fixture::FixtureLibrary::open(directory.join("fixtures.sqlite"))
            .expect("library");
        library
            .load_fixture_package_directory(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library"),
            )
            .expect("the shipped packages load");
        let path = directory.join("packaged-demo.show");
        viz_demo::generate(library, &path).expect("the demo generates");
        path
    }

    #[test]
    fn opening_the_demo_opens_a_copy_and_never_the_packaged_template() {
        let directory = workspace("open");
        let template = template(&directory);
        let original = std::fs::read(&template).expect("template bytes");
        let shows = directory.join("shows");
        std::fs::create_dir_all(&shows).expect("shows folder");
        let session = Session::default();

        let first = open_copy(&session, &template, &shows).expect("the demo opens");
        assert_eq!(first.name, "Demo Show");
        assert_ne!(
            Path::new(&first.path),
            template,
            "the packaged template itself was opened"
        );
        assert!(first.fixture_count > 0, "the copy carries the demo rig");

        // Opening it again is another document, not the same one reopened.
        let second = open_copy(&session, &template, &shows).expect("the demo opens again");
        assert_eq!(second.name, "Demo Show 2");
        assert_ne!(second.path, first.path);
        assert_eq!(second.fixture_count, first.fixture_count);

        assert_eq!(
            std::fs::read(&template).expect("template bytes"),
            original,
            "opening the demo modified the packaged template"
        );
    }

    /// A copy that cannot be opened must not be left behind for the operator to find.
    #[test]
    fn a_copy_that_will_not_open_is_cleaned_up() {
        let directory = workspace("broken");
        let template = directory.join("broken.show");
        std::fs::write(&template, b"not a show file").expect("broken template");
        let shows = directory.join("shows");
        std::fs::create_dir_all(&shows).expect("shows folder");

        let session = Session::default();
        assert!(open_copy(&session, &template, &shows).is_err());
        assert!(
            !shows.join("demo-show.show").exists(),
            "a copy that would not open was left in the shows folder"
        );
    }
}
