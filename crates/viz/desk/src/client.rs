//! Authenticated read-only HTTP client for the desk API.

use crate::wire::{
    ObjectCollection, PatchSnapshot, Readiness, SessionResponse, VisualizerViewSnapshot,
};
use std::time::Duration;
use viz_scene::ProviderError;

pub struct DeskClient {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
    session_id: Option<uuid::Uuid>,
}

impl DeskClient {
    pub fn new(host: &str, port: u16) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| ProviderError::new("http client", error.to_string(), false))?;
        Ok(Self {
            http,
            base: format!("http://{host}:{port}"),
            token: None,
            session_id: None,
        })
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub async fn readiness(&self) -> Result<Readiness, ProviderError> {
        self.get_json("/api/v2/readiness", "readiness").await
    }

    /// Open the read-only visualizer session.
    ///
    /// Servers that predate the role fall back to a normal session; the renderer still never
    /// issues a mutating request, but it says so in diagnostics.
    pub async fn open_session(&mut self, user: &str) -> Result<bool, ProviderError> {
        let response = self
            .http
            .post(format!("{}/api/v2/sessions", self.base))
            .json(&serde_json::json!({"username": user, "role": "visualizer"}))
            .send()
            .await
            .map_err(|error| ProviderError::new("authentication", error.to_string(), true))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::new(
                "authentication",
                format!("{status}: {body}"),
                status.is_server_error(),
            ));
        }
        let session: SessionResponse = response
            .json()
            .await
            .map_err(|error| ProviderError::new("authentication", error.to_string(), false))?;
        let read_only = session.role.as_deref() == Some("visualizer");
        self.token = Some(session.token);
        self.session_id = Some(session.session_id);
        Ok(read_only)
    }

    pub async fn close_session(&mut self) {
        let (Some(token), Some(id)) = (self.token.clone(), self.session_id) else {
            return;
        };
        let _ = self
            .http
            .delete(format!("{}/api/v2/sessions/{id}", self.base))
            .bearer_auth(token)
            .send()
            .await;
        self.token = None;
        self.session_id = None;
    }

    pub async fn patch(&self) -> Result<PatchSnapshot, ProviderError> {
        self.get_json("/api/v2/patch", "patch snapshot").await
    }

    /// What the desk is telling its renderers to look at.
    ///
    /// A desk that predates the view answers 404, which is not a failure: the renderer keeps the
    /// view the operator selected, exactly as it did before there was anything to follow.
    pub async fn visualizer_views(&self) -> Option<VisualizerViewSnapshot> {
        self.get_json::<VisualizerViewSnapshot>("/api/v2/visualizer-views", "visualizer view")
            .await
            .ok()
    }

    /// The preview values a planning window is driving, if this source has any.
    ///
    /// A lighting desk answers 404 and that is the correct answer, not a failure: the two-plane
    /// rule says a desk's live values arrive as real Art-Net or sACN. So this is `None` for a desk
    /// and the renderer simply never has a preview plane to merge.
    pub async fn preview_values(&self) -> Option<crate::wire::PreviewSnapshot> {
        self.get_json::<crate::wire::PreviewSnapshot>("/api/v2/preview-values", "preview values")
            .await
            .ok()
    }

    /// The desk's own live output universes.
    ///
    /// For a renderer drawing the desk's Stage inside the desk's own window. It is the same
    /// numbers the desk is sending, read from the desk rather than heard from the network, because
    /// there may be no network: a desk with no output routes still has a Stage to draw.
    pub async fn output_dmx(&self) -> Option<crate::wire::OutputDmxSnapshot> {
        self.get_json::<crate::wire::OutputDmxSnapshot>("/api/v2/output/dmx", "desk output")
            .await
            .ok()
    }

    pub async fn objects(&self, kind: &str) -> Result<ObjectCollection, ProviderError> {
        self.get_json(&format!("/api/v2/objects/{kind}"), "show objects")
            .await
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        boundary: &'static str,
    ) -> Result<T, ProviderError> {
        let mut request = self.http.get(format!("{}{path}", self.base));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ProviderError::new(boundary, error.to_string(), true))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::new(
                boundary,
                format!("{status}: {body}"),
                status.is_server_error() || status.as_u16() == 409,
            ));
        }
        response
            .json()
            .await
            .map_err(|error| ProviderError::new(boundary, error.to_string(), false))
    }
}
