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

use std::net::SocketAddr;
use std::path::PathBuf;
use tauri::Manager;

/// Where to serve the open document for a visualizer that launched this window.
///
/// The visualizer picks the port and passes it, because it is the one that has to connect. With
/// no address the editor simply runs on its own, which is what happens when an operator opens it
/// from the applications folder.
fn scene_address() -> Option<SocketAddr> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--serve" {
            return arguments.next().and_then(|value| value.parse().ok());
        }
    }
    std::env::var("TOSKLIGHT_VIZ_SCENE_ADDRESS")
        .ok()
        .and_then(|value| value.parse().ok())
}

/// Where the shipped fixture packages live, so the fixture browser has something to offer.
fn fixture_library_path(app: &tauri::App) -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("LIGHT_FIXTURE_LIBRARY") {
        return Some(PathBuf::from(configured));
    }
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("fixture-library"));
    if let Some(path) = bundled.filter(|path| path.exists()) {
        return Some(path);
    }
    // Development: the desk's own runtime library, if this checkout has one.
    option_env!("LIGHT_RUNTIME_DATA_DIR")
        .map(PathBuf::from)
        .map(|dir| dir.join("fixtures.sqlite"))
        .filter(|path| path.exists())
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
        .invoke_handler(tauri::generate_handler![
            session::create_document,
            session::open_document,
            demo::open_demo_show,
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
            let library = fixture_library_path(app);
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run the ToskLight Viz editor")
}
