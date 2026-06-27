use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

/// Wrap a serializable value in the standard success envelope: `{"data": ...}`.
pub fn data<T: Serialize>(t: T) -> Json<Value> {
    Json(json!({ "data": t }))
}

/// Wrap a collection and its pagination metadata: `{"data": ..., "meta": ...}`.
pub fn data_paginated<T: Serialize>(items: T, meta: Value) -> Json<Value> {
    Json(json!({ "data": items, "meta": meta }))
}
