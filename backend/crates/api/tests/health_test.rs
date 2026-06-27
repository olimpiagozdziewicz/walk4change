use axum::http::StatusCode;
use tower::ServiceExt; // oneshot

#[tokio::test]
async fn health_returns_ok() {
    let app = walk4change_api::router_health();
    let resp = app
        .oneshot(axum::http::Request::builder().uri("/api/v1/health").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
