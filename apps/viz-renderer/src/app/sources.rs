//! Active source selection, provider connection and owned child-process lifecycle.

use super::{Application, SourceAuthority};
use crate::session::Session;
use std::time::{Duration, Instant};
use viz_desk::{DeskConnection, DeskProvider};
use viz_scene::{ConnectionState, ProviderKind};

pub(super) fn canonical_demo_show_path() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("TOSKLIGHT_VIZ_DEMO_SHOW")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
    {
        return path
            .is_file()
            .then_some(path.clone())
            .ok_or_else(|| format!("{} is not a file", path.display()));
    }
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets/demo.show");
    if checkout.is_file() {
        return Ok(checkout);
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("resolve visualizer executable: {error}"))?;
    let parent = executable
        .parent()
        .ok_or_else(|| "visualizer executable has no parent directory".to_owned())?;
    for candidate in [
        parent.join("demo-show/demo-show.show"),
        parent.join("demo-show/demo.show"),
        parent.join("../Resources/demo-show/demo-show.show"),
        parent.join("../Resources/demo-show/demo.show"),
    ] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err("this build has no assets/demo.show or packaged demo-show/demo-show.show".to_owned())
}

impl Application {
    pub(super) fn connection_endpoint(
        &self,
        hosted_port: Option<u16>,
        planner_port: Option<u16>,
    ) -> (String, u16) {
        let local_port = match self.source_authority {
            SourceAuthority::External => None,
            SourceAuthority::LocalShow => hosted_port,
            SourceAuthority::LocalPlanner => planner_port,
        };
        local_port.map_or_else(
            || (self.preferences.host.clone(), self.preferences.port),
            |port| ("127.0.0.1".into(), port),
        )
    }

    pub(super) fn start_session(&mut self) {
        let (provider, kind) = self.build_provider();
        self.session = Some(Session::new(provider, kind, self.epoch));
    }

    /// Build the provider the current preferences select. Demo mode hosts the canonical show file
    /// through the same path as every other standalone show.
    pub(super) fn build_provider(&mut self) -> (Box<dyn viz_scene::SceneProvider>, ProviderKind) {
        // Started by the desk: everything drawn arrives over the channel on stdin, and this
        // process chooses nothing. Taken once — the pipe cannot be read from twice — so a rebuild
        // after the desk has gone falls through to the built-in scene rather than hanging on a
        // channel nobody is writing to.
        if self.options.helper
            && let Some(source) = self.helper_source.take()
        {
            return (Box::new(source), ProviderKind::LightingDesk);
        }
        // Only a locally selected planner may launch a new editor. Explicit Connect uses its
        // chosen endpoint even while a previously opened document remains recoverable.
        if self.source_authority == SourceAuthority::LocalPlanner
            && self.preferences.source == ProviderKind::PlanningSoftware
            && self.planning_window.is_none()
            && !self.options.planning_server_requested
        {
            match crate::planner::PlanningWindow::open() {
                Ok(window) => {
                    self.planning_window = Some(window);
                    self.lasting_failure = None;
                }
                Err(error) => {
                    eprintln!("open the planning window: {error}");
                    self.lasting_failure = Some(format!("planning window: {error}"));
                }
            }
        }
        // Inactive local resources do not override an explicitly chosen external endpoint.
        let (host, port) = self.connection_endpoint(
            self.hosted_show
                .as_ref()
                .map(crate::showfile::HostedShow::port),
            self.planning_window.as_ref().map(|window| window.port()),
        );
        (
            Box::new(DeskProvider::start(
                DeskConnection {
                    host,
                    port,
                    input_overrides: self.preferences.applied_input_overrides(
                        self.source_authority == SourceAuthority::LocalShow,
                    ),
                    listen_interfaces: self.preferences.listen_interfaces.clone(),
                    target: self.options.target.clone(),
                    ..DeskConnection::default()
                },
                self.epoch,
            )),
            ProviderKind::LightingDesk,
        )
    }

    /// Open a show file: start its private server, then point the session at it.
    pub fn open_show_file(&mut self, path: &std::path::Path) {
        match crate::showfile::HostedShow::open(path) {
            Ok(hosted) => {
                self.hosted_show = Some(hosted);
                self.source_authority = SourceAuthority::LocalShow;
                self.lasting_failure = None;
                self.framed_revision = None;
                self.camera_is_local = false;
                self.reconnect();
            }
            Err(error) => {
                if let Some(session) = self.session.as_mut() {
                    session.connection = viz_scene::ConnectionState::Failed {
                        boundary: path.display().to_string(),
                        detail: error.clone(),
                    };
                }
                eprintln!("open show file: {error}");
            }
        }
    }

    /// Stop only private sources this renderer owns, including a retained inactive preview.
    pub(crate) fn shutdown_owned_sources(&mut self) {
        // Native termination need not unwind Application; the exiting callback owns cleanup.
        self.hosted_show.take();
        self.planning_window.take();
        if let Some(session) = self.session.as_mut() {
            session.shutdown();
        }
    }

    /// Close an opened show file and return to the desk the preferences name.
    pub fn close_show_file(&mut self) {
        self.lasting_failure = None;
        let closed = self.hosted_show.take().is_some();
        let changed_source = self.source_authority != SourceAuthority::External;
        self.source_authority = SourceAuthority::External;
        if closed || changed_source {
            self.framed_revision = None;
            self.camera_is_local = false;
            self.reconnect();
        }
    }

    /// Ask the operator for a show file and open it.
    pub fn prompt_for_show_file(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open Show File")
            .add_filter("ToskLight show", &["show"]);
        if let Some(hosted) = self.hosted_show.as_ref()
            && let Some(parent) = hosted.path().parent()
        {
            dialog = dialog.set_directory(parent);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_show_file(&path);
        }
    }

    /// Open the product's rig-planning window and make its document the rendered source.
    pub(super) fn open_rig_editor(&mut self) {
        self.lasting_failure = None;
        if self.planning_window.is_none() {
            match crate::planner::PlanningWindow::open() {
                Ok(window) => self.planning_window = Some(window),
                Err(error) => {
                    self.lasting_failure = Some(format!("rig editor: {error}"));
                    return;
                }
            }
        }
        self.source_authority = SourceAuthority::LocalPlanner;
        self.preferences.source = ProviderKind::PlanningSoftware;
        self.framed_revision = None;
        self.camera_is_local = false;
        self.reconnect();
    }

    /// Notice a private server or planning window that has exited.
    ///
    /// Either one dying looks exactly like a slow connection from the inside, and an operator
    /// staring at an empty picture deserves to be told which process is gone rather than left
    /// watching a connection retry something that will never answer again.
    pub(super) fn watch_children(&mut self) {
        let now = Instant::now();
        if now < self.next_child_check {
            return;
        }
        self.next_child_check = now + Duration::from_secs(1);
        let mut gone: Option<(String, String)> = None;
        if let Some(hosted) = self.hosted_show.as_mut()
            && hosted.exited()
        {
            if self.source_authority == SourceAuthority::LocalShow {
                gone = Some((
                    hosted.label(),
                    "the private server for this show file exited".to_owned(),
                ));
            }
            self.hosted_show = None;
        }
        if let Some(planning) = self.planning_window.as_mut()
            && planning.exited()
        {
            if self.source_authority == SourceAuthority::LocalPlanner {
                gone = Some((
                    "planning window".to_owned(),
                    "the Viz editor was closed; open a show file or connect to a desk".to_owned(),
                ));
            }
            self.planning_window = None;
        }
        if let Some((boundary, detail)) = gone {
            // The connection keeps its own states, and it will go on reporting a refused socket
            // over the top of this, so the reason is kept where it stays put until it is dealt
            // with. The picture that was last drawn stays on screen underneath it.
            self.lasting_failure = Some(format!("{boundary}: {detail}"));
            if let Some(session) = self.session.as_mut() {
                session.connection = ConnectionState::Failed { boundary, detail };
            }
        }
    }

    /// Stage a new connection. The current scene stays on screen until the candidate validates.
    pub(super) fn reconnect(&mut self) {
        let (provider, kind) = self.build_provider();
        if let Some(session) = self.session.as_mut() {
            session.replace_provider(provider, kind);
        } else {
            self.session = Some(Session::new(provider, kind, self.epoch));
        }
    }
}
