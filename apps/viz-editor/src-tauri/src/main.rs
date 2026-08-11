#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! The ToskLight Viz editor.
//!
//! A rig-planning window: the same patch sheet the desk uses, over a planning document rather
//! than a running desk. It starts no server, joins no desk session, and outputs no DMX. The
//! visualizer renders whatever this document describes, lit by whatever console is actually on
//! the network.

mod contract;
mod demo;
mod discovery;
mod recent;
mod session;
mod verify;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

enum FixtureLibrarySource {
    Packages(PathBuf),
    Database(PathBuf),
}

/// Where to serve the open document for a visualizer that launched this window.
///
/// The visualizer picks the port and passes it, because it is the one that has to connect. With
/// no address the editor simply runs on its own, which is what happens when an operator opens it
/// from the applications folder.
fn scene_address() -> Option<SocketAddr> {
    served_address(std::env::args().skip(1)).or_else(|| {
        std::env::var("TOSKLIGHT_VIZ_SCENE_ADDRESS")
            .ok()
            .and_then(|value| value.parse().ok())
    })
}

/// The address `--serve` names, read from arguments rather than from the process, so the rule can
/// be checked without a process that has them.
fn served_address(arguments: impl IntoIterator<Item = String>) -> Option<SocketAddr> {
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--serve" {
            return arguments.next().and_then(|value| value.parse().ok());
        }
    }
    None
}

/// Whether the visualizer opened this window, rather than an operator opening the editor itself.
///
/// One product means one Dock tile. Opened on its own the editor is an application an operator
/// chose and keeps its own tile; opened by the visualizer it is a window of that application, and a
/// second tile for the same product is what an operator reads as two programs running.
///
/// Implied by `--serve`, which only the visualizer passes: it is how the visualizer says where to
/// serve the document it is about to connect to, so nothing else has a reason to send it.
#[cfg_attr(
    not(target_os = "macos"),
    allow(dead_code, reason = "one Dock tile is a macOS idea")
)]
fn opened_by_the_visualizer() -> bool {
    scene_address().is_some()
}

/// Where the shipped fixture packages live, so the fixture browser has something to offer.
fn fixture_library_source(app: &tauri::App) -> Result<Option<FixtureLibrarySource>, String> {
    if let Some(configured) = std::env::var_os("LIGHT_FIXTURE_LIBRARY") {
        let path = PathBuf::from(configured);
        return if path.is_dir() {
            Ok(Some(FixtureLibrarySource::Packages(path)))
        } else if path.is_file() {
            Ok(Some(FixtureLibrarySource::Database(path)))
        } else {
            Err(format!(
                "LIGHT_FIXTURE_LIBRARY names {}, which is neither a fixture-package directory nor a fixture database",
                path.display()
            ))
        };
    }
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("fixture-library"));
    if let Some(path) = bundled.filter(|path| path.exists()) {
        return Ok(Some(FixtureLibrarySource::Packages(path)));
    }
    // Development: the desk's own runtime library, if this checkout has one.
    Ok(option_env!("LIGHT_RUNTIME_DATA_DIR")
        .map(PathBuf::from)
        .map(|dir| dir.join("fixtures.sqlite"))
        .filter(|path| path.exists())
        .map(FixtureLibrarySource::Database))
}

fn prepare_fixture_library(
    source: Option<FixtureLibrarySource>,
    app_data: &std::path::Path,
) -> Result<Option<PathBuf>, String> {
    let Some(source) = source else {
        return Ok(None);
    };
    match source {
        FixtureLibrarySource::Database(path) => Ok(Some(path)),
        FixtureLibrarySource::Packages(packages) => {
            std::fs::create_dir_all(app_data).map_err(|error| {
                format!(
                    "could not create writable pre-visualizer data directory {}: {error}",
                    app_data.display()
                )
            })?;
            let database = app_data.join("fixtures.sqlite");
            let library = light_fixture::FixtureLibrary::open(&database).map_err(|error| {
                format!(
                    "could not open writable fixture database {}: {error}",
                    database.display()
                )
            })?;
            library
                .load_fixture_package_directory(&packages)
                .map_err(|error| {
                    format!(
                        "could not load bundled fixture packages from {} into {}: {error}",
                        packages.display(),
                        database.display()
                    )
                })?;
            Ok(Some(database))
        }
    }
}

/// Serve this editor's document to the network, and say where.
///
/// The visualizer that launched this window gets its own loopback address on the command line;
/// this is the other listener, the one a desk elsewhere in the building can reach when the
/// operator asks it to load what is planned here. It is read-only — the same routes the renderer
/// uses — and it costs one port.
fn announce_on_the_network(app: &tauri::App) {
    let session = app.state::<session::Session>();
    let discovery = app.state::<discovery::Discovery>();
    if let Ok(data) = app.path().app_data_dir() {
        discovery.set_downloads(data.join("shows"));
    }
    let source = session.scene_source();
    let bound =
        tauri::async_runtime::block_on(viz_planning::bind(SocketAddr::from(([0, 0, 0, 0], 0))));
    let (listener, address) = match bound {
        Ok(bound) => bound,
        Err(error) => {
            eprintln!("this editor will not be reachable on the network: {error}");
            return;
        }
    };
    tauri::async_runtime::spawn(async move {
        if let Err(error) = viz_planning::serve_on(source, listener).await {
            eprintln!("document server on {address}: {error}");
        }
    });
    discovery.start(address.port(), session.document_name());
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(session::Session::default())
        .manage(discovery::Discovery::default())
        .manage(Arc::new(verify::SurfaceReady::default()))
        .invoke_handler(tauri::generate_handler![
            session::create_document,
            session::open_document,
            demo::open_demo_show,
            verify::surface_ready,
            session::document_summary,
            session::save_document_as,
            session::rename_document,
            session::patch_snapshot,
            session::patch_fixtures,
            session::library_profiles,
            session::set_preview,
            session::clear_preview,
            session::preview_is_active,
            session::patch_layers,
            session::save_patch_layer,
            session::export_mvr,
            session::preview_mvr,
            session::import_mvr,
            discovery::discovered_desks,
            discovery::load_from_desk,
        ])
        .setup(|app| {
            // Before the window is shown, so the tile never appears and then disappears.
            #[cfg(target_os = "macos")]
            if opened_by_the_visualizer() {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            let source = fixture_library_source(app).map_err(std::io::Error::other)?;
            let app_data = app.path().app_data_dir().map_err(std::io::Error::other)?;
            let library =
                prepare_fixture_library(source, &app_data).map_err(std::io::Error::other)?;
            let session = app.state::<session::Session>();
            session.set_library_path(library);
            if let Ok(config) = app.path().app_config_dir() {
                session.set_recent_store(recent::RecentShow::at(config.join("recent-show")));
                session.reopen_recent();
            }
            announce_on_the_network(app);
            if let Some(address) = scene_address() {
                let source = session.scene_source();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = viz_planning::serve(source, address).await {
                        eprintln!("scene source on {address}: {error}");
                    }
                });
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            // `--verify` opens the window, waits for the interface to report itself, and exits
            // with the verdict. Nothing in the build catches a window that opens white.
            if verify::requested() {
                verify::watch(app.state::<Arc<verify::SurfaceReady>>().inner().clone());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run the ToskLight Viz editor")
}

#[cfg(test)]
mod tests {
    use super::{FixtureLibrarySource, prepare_fixture_library, served_address};
    use std::path::PathBuf;

    fn workspace(name: &str) -> PathBuf {
        let root = std::env::var_os("LIGHT_TMP_DIR").map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp"),
            PathBuf::from,
        );
        let directory = root
            .join("viz-editor-library-tests")
            .join(format!("{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn shipped_packages() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library")
    }

    #[test]
    fn bundled_packages_populate_only_a_clean_writable_app_data_database() {
        let root = workspace("packaged-clean");
        let app_data = root.join("Application Support");
        let packages = shipped_packages();

        let database = prepare_fixture_library(
            Some(FixtureLibrarySource::Packages(packages.clone())),
            &app_data,
        )
        .expect("packaged fixture library")
        .expect("fixture database path");

        assert_eq!(database, app_data.join("fixtures.sqlite"));
        assert!(!database.starts_with(&packages));
        let library = light_fixture::FixtureLibrary::open(&database).expect("writable database");
        let profiles = library.profiles().expect("installed profiles");
        assert!(
            profiles.len() > 40,
            "the normal shipped library is available"
        );
        assert!(profiles.iter().any(|profile| {
            profile.manufacturer == "Generic" && profile.name == "Dimmer Profile"
        }));

        prepare_fixture_library(Some(FixtureLibrarySource::Packages(packages)), &app_data)
            .expect("repeat startup is idempotent");
        assert_eq!(
            light_fixture::FixtureLibrary::open(&database)
                .unwrap()
                .profiles()
                .unwrap()
                .len(),
            profiles.len()
        );
    }

    #[test]
    fn corrupt_packaged_resources_name_both_source_and_database() {
        let root = workspace("packaged-corrupt");
        let packages = root.join("Resources/fixture-library");
        std::fs::create_dir_all(&packages).unwrap();
        std::fs::write(packages.join("broken.toskfixture"), b"not a package").unwrap();
        let app_data = root.join("Application Support");

        let error = prepare_fixture_library(
            Some(FixtureLibrarySource::Packages(packages.clone())),
            &app_data,
        )
        .expect_err("corrupt package is actionable");
        assert!(error.contains(&packages.display().to_string()));
        assert!(error.contains(&app_data.join("fixtures.sqlite").display().to_string()));
    }

    /// One product, one Dock tile. The editor keeps its own only when an operator opened it: what
    /// distinguishes the two is the address the visualizer passes so it can connect to the
    /// document, which nothing else has a reason to send.
    #[test]
    fn only_a_visualizer_launch_names_a_document_to_serve() {
        fn arguments(values: &[&str]) -> Vec<String> {
            values.iter().map(|value| (*value).to_owned()).collect()
        }
        assert_eq!(
            served_address(arguments(&["--serve", "127.0.0.1:51234"])),
            Some("127.0.0.1:51234".parse().expect("an address")),
        );
        assert_eq!(served_address(arguments(&[])), None, "opened on its own");
        assert_eq!(
            served_address(arguments(&["--verify"])),
            None,
            "a verification run is still the editor opened by itself"
        );
        assert_eq!(
            served_address(arguments(&["--serve"])),
            None,
            "an address that was never given is no address"
        );
        assert_eq!(
            served_address(arguments(&["--serve", "not an address"])),
            None,
        );
    }
}
