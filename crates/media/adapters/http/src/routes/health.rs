//! What the process is running, and the library it published.

use axum::extract::State;
use axum::response::IntoResponse;

use crate::routes::ApiState;
use crate::wire::{CatalogView, Health};

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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

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
    async fn the_catalog_is_served_as_the_same_snapshot_everything_else_reads() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/catalog".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["revision"], 0);
        assert_eq!(body["itemCount"], 0);
        assert!(body["folders"].as_array().unwrap().is_empty());
    }
}
