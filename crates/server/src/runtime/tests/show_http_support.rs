async fn create_show(app: &Router, token: &str, name: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shows")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({"name":name,"data_base64":null,"overwrite":false})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await
}
