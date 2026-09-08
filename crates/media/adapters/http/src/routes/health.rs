//! What the process is running, and the library it published.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::routes::ApiState;
use crate::wire::{CatalogView, Health, RunningServerView};

pub(super) async fn health(State(state): State<ApiState>) -> impl IntoResponse {
    let catalog = state.catalog.load();
    axum::Json(Health {
        status: "ok".to_owned(),
        instance: state.configuration.load().instance_id.as_str().to_owned(),
        outputs: state.state.load().outputs.len(),
        catalog_revision: catalog.revision.value(),
        catalog_items: catalog.item_count(),
    })
}

pub(super) async fn catalog(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(CatalogView::of(&state.catalog.load()))
}

pub(super) async fn runtime(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(RunningServerView::of(
        &state.active_configuration,
        &state.administration_endpoint,
        &state.configuration_path,
        state.data_directory.as_deref(),
    ))
}

pub(super) async fn open_data_directory(
    State(state): State<ApiState>,
) -> Result<StatusCode, crate::ApiError> {
    (state.open_data_directory)().map_err(|error| {
        tracing::error!(%error, "the portable Media Server folder could not be opened");
        crate::ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "data-directory-not-opened",
            "The folder could not be opened on the Media Server computer.",
        )
    })?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use media_application::MediaConfiguration;

    use crate::routes::bench::{bench, get, send};

    #[tokio::test]
    async fn health_reports_what_the_process_is_running() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/health".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["outputs"], 1);
    }

    #[tokio::test]
    async fn runtime_reports_startup_facts_not_pending_configuration_edits() {
        let bench = bench();
        bench.configuration.rcu(|configuration| {
            let mut changed = MediaConfiguration::clone(configuration);
            changed.outputs[0].universe = 42;
            changed.outputs[0].start_address = 321;
            changed
        });

        let (status, body) = send(&bench.router, get("/api/v2/runtime".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["administrationIp"], "127.0.0.1");
        assert_eq!(body["outputs"][0]["universe"], 0);
        assert_eq!(body["outputs"][0]["startAddress"], 1);
        assert_eq!(body["dataDirectory"], "/tmp/tosklight-media");
        assert_eq!(body["libraryDirectory"], "/tmp/tosklight-media/library");
        assert_eq!(
            body["configurationFile"],
            "/tmp/tosklight-media/media-server.json"
        );
        assert_eq!(body["portable"], true);
    }

    #[tokio::test]
    async fn opening_the_data_directory_uses_the_process_owned_action() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            crate::routes::bench::post("/api/v2/runtime/data-directory/open".into(), "{}"),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn the_catalog_is_served_as_the_same_snapshot_everything_else_reads() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/catalog".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["revision"], 0);
        assert_eq!(body["itemCount"], 0);
        assert!(body["folders"].as_array().unwrap().is_empty());
    }
}
