//! Persisted effect presets selected by the two live layer banks.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_domain::EffectSlot;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{EffectPresetView, UpdateEffectPreset};

pub(super) async fn effects(State(state): State<ApiState>) -> impl IntoResponse {
    let configuration = state.configuration.load();
    axum::Json(
        configuration
            .effects
            .entries
            .iter()
            .map(EffectPresetView::of)
            .collect::<Vec<_>>(),
    )
}

pub(super) async fn update_effect(
    State(state): State<ApiState>,
    Path(slot): Path<u8>,
    TolerantJson(body): TolerantJson<UpdateEffectPreset>,
) -> Result<Response, ApiError> {
    if slot == 0 {
        return Err(ApiError::bad_request(
            "reserved-effect-slot",
            "effect slot 0 is the fixed Off value",
        ));
    }
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());

    if body.clear.unwrap_or(false) {
        configuration.effects.remove(slot);
        return edit::commit(
            &state,
            configuration,
            &body.request_id,
            &serde_json::json!({ "slot": slot, "assigned": false }),
        );
    }

    let existing = configuration.effects.resolve(slot).cloned();
    let name = body
        .name
        .or_else(|| existing.as_ref().map(|entry| entry.name.clone()))
        .unwrap_or_else(|| format!("Effect {slot}"));
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request(
            "empty-name",
            "an effect preset needs a name an operator can find it by",
        ));
    }
    let effect_type = body
        .effect_type
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|entry| entry.effect.effect_type.clone())
        })
        .ok_or_else(|| {
            ApiError::bad_request(
                "missing-effect-type",
                "assign an effect type when creating a preset",
            )
        })?;
    let mut effect = existing.map(|entry| entry.effect).unwrap_or_default();
    if effect.effect_type.as_deref() != Some(effect_type.as_str()) {
        effect = EffectSlot {
            effect_type: Some(effect_type),
            enabled: true,
            mix: 1.0,
            seed: u32::from(slot),
            ..EffectSlot::default()
        };
    }
    if let Some(parameters) = body.parameters {
        if parameters.iter().any(|value| !value.is_finite()) {
            return Err(ApiError::bad_request(
                "invalid-effect-parameters",
                "effect parameters must be finite numbers",
            ));
        }
        effect.parameters = parameters;
    }
    effect.normalize();
    configuration
        .effects
        .assign(slot, name.to_owned(), effect)
        .map_err(|error| ApiError::bad_request("effect-not-assigned", error.to_string()))?;
    let view = EffectPresetView::of(
        configuration
            .effects
            .resolve(slot)
            .expect("the assigned effect is immediately addressable"),
    );
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn the_seeded_effect_catalog_is_addressed_and_operator_named() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/effects".into())).await;
        assert_eq!(status, StatusCode::OK);
        let entries = body.as_array().expect("an effect list");
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0]["slot"], 1);
        assert!(
            entries
                .iter()
                .any(|entry| entry["name"] == "TV/CRT/VHS Simulation")
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry["name"] == "Digital Video/ Glitch Simulation")
        );
        assert!(entries.iter().any(|entry| entry["name"] == "B/W Rasterize"));
        assert!(
            entries
                .iter()
                .any(|entry| entry["name"] == "CMYK Rasterize")
        );
    }

    #[tokio::test]
    async fn editing_and_clearing_a_slot_is_persisted_before_it_is_answered() {
        let bench = bench();
        let (status, changed) = send(
            &bench.router,
            post(
                "/api/v2/effects/42/update".into(),
                r#"{"requestId":"effect-42","name":"House tunnel","effectType":"feedback","parameters":[0.93,0.4,7]}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(changed["slot"], 42);
        assert_eq!(changed["name"], "House tunnel");
        assert_eq!(bench.stored.lock().unwrap().len(), 1);
        assert_eq!(
            bench.stored.lock().unwrap()[0]
                .effects
                .resolve(42)
                .map(|entry| entry.name.as_str()),
            Some("House tunnel")
        );

        let (status, cleared) = send(
            &bench.router,
            post(
                "/api/v2/effects/42/update".into(),
                r#"{"requestId":"clear-42","clear":true}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cleared["assigned"], false);
        assert_eq!(bench.stored.lock().unwrap().len(), 2);
        assert!(
            bench.stored.lock().unwrap()[1]
                .effects
                .resolve(42)
                .is_none()
        );
    }

    #[tokio::test]
    async fn off_is_not_an_assignable_library_slot() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/effects/0/update".into(),
                r#"{"requestId":"bad","name":"Not off","effectType":"blur"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "reserved-effect-slot");
        assert!(bench.stored.lock().unwrap().is_empty());
    }
}
