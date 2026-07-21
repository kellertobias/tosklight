use schemars::{JsonSchema, generate::SchemaSettings};

use crate::v2::output_runtime::{
    OutputRuntimeActionOutcome, OutputRuntimeActionRequest, OutputRuntimeErrorResponse,
};

use super::{GeneratedArtifact, namespaced_schema};

const SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-output";

pub(super) fn artifacts() -> Vec<GeneratedArtifact> {
    vec![
        request_schema::<OutputRuntimeActionRequest>("output-runtime-action-request"),
        response_schema::<OutputRuntimeActionOutcome>("output-runtime-action-outcome"),
        response_schema::<OutputRuntimeErrorResponse>("output-runtime-error-response"),
    ]
}

fn request_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    namespaced_schema::<T>(
        SCHEMA_DIRECTORY,
        name,
        SchemaSettings::draft2020_12().for_deserialize(),
    )
}

fn response_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    namespaced_schema::<T>(
        SCHEMA_DIRECTORY,
        name,
        SchemaSettings::draft2020_12().for_serialize(),
    )
}
