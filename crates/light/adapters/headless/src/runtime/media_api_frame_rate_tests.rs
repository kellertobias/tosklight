//! The Media Server's In/Out point frame rate, as the desk's native Media snapshot reads it from
//! `GET /api/v2/outputs`. The same fields are asserted against the server in
//! `crates/media/adapters/http/src/routes/desk_contract_tests.rs`.

use super::NativeMediaOutputResponse;

fn read(json: &str) -> NativeMediaOutputResponse {
    serde_json::from_str(json).unwrap()
}

#[test]
fn an_output_reads_its_point_frame_rate_and_an_older_server_reports_none() {
    let current =
        read(r#"{"id":"A","frameRate":30,"layers":[{"effects":[],"visualizerChannels":[]}]}"#);
    assert_eq!(current.frame_rate, Some(30));
    assert_eq!(current.layers.len(), 1);

    let older = read(r#"{"id":"A","layers":[{"effects":[]}]}"#);
    assert_eq!(older.frame_rate, None);
}
