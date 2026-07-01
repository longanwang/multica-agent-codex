use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct RelayState {
    tenants: Arc<DashMap<String, TenantChannel>>,
    shared_secret: Option<String>,
}

#[derive(Clone)]
struct TenantChannel {
    tx: broadcast::Sender<RelayEnvelope>,
    queue: Arc<Mutex<VecDeque<RelayEnvelope>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayEnvelope {
    id: String,
    connector: String,
    tenant_id: String,
    body_hash: String,
    payload: Value,
    received_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplyRequest {
    connector: String,
    conversation_id: String,
    text: String,
    in_reply_to: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "multica_relay=info,tower_http=info".to_string()),
        )
        .init();

    let state = RelayState {
        tenants: Arc::new(DashMap::new()),
        shared_secret: std::env::var("MULTICA_RELAY_SECRET").ok(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/desktop/ws/{tenant_id}", get(desktop_ws))
        .route("/desktop/replies/{tenant_id}", post(desktop_reply))
        .route("/webhooks/feishu/{tenant_id}", post(feishu_webhook))
        .route("/webhooks/wecom/{tenant_id}", post(wecom_webhook))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = std::env::var("MULTICA_RELAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()?;
    tracing::info!(%addr, "starting Multica relay");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(json!({"ok": true, "service": "multica-relay"}))
}

async fn feishu_webhook(
    State(state): State<RelayState>,
    Path(tenant_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    ingest_webhook(state, tenant_id, "feishu", headers, body).await
}

async fn wecom_webhook(
    State(state): State<RelayState>,
    Path(tenant_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    ingest_webhook(state, tenant_id, "wecom", headers, body).await
}

async fn ingest_webhook(
    state: RelayState,
    tenant_id: String,
    connector: &'static str,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if !verify_shared_secret(&state, &headers, &body) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid signature"}))).into_response();
    }

    let payload: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("invalid JSON body: {error}")})),
            )
                .into_response();
        }
    };

    if let Some(challenge) = challenge_response(&payload) {
        return Json(json!({ "challenge": challenge })).into_response();
    }

    let envelope = RelayEnvelope {
        id: uuid::Uuid::new_v4().to_string(),
        connector: connector.to_string(),
        tenant_id: tenant_id.clone(),
        body_hash: sha256_hex(&body),
        payload,
        received_at: Utc::now(),
    };
    let channel = state.channel_for(&tenant_id);
    channel.enqueue(envelope.clone());
    let _ = channel.tx.send(envelope.clone());

    Json(json!({
        "ok": true,
        "id": envelope.id,
        "bodyHash": envelope.body_hash
    }))
    .into_response()
}

async fn desktop_ws(
    State(state): State<RelayState>,
    Path(tenant_id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| desktop_socket(socket, state, tenant_id))
}

async fn desktop_socket(socket: WebSocket, state: RelayState, tenant_id: String) {
    let channel = state.channel_for(&tenant_id);
    let mut rx = channel.tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    let queued = channel.snapshot_queue();
    for envelope in queued {
        if sender
            .send(Message::Text(serde_json::to_string(&envelope).unwrap_or_default().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    loop {
        tokio::select! {
            received = rx.recv() => {
                match received {
                    Ok(envelope) => {
                        if sender
                            .send(Message::Text(serde_json::to_string(&envelope).unwrap_or_default().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => continue,
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn desktop_reply(
    State(state): State<RelayState>,
    Path(tenant_id): Path<String>,
    headers: HeaderMap,
    Json(reply): Json<ReplyRequest>,
) -> impl IntoResponse {
    if !verify_shared_secret(&state, &headers, reply.text.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid signature"}))).into_response();
    }

    Json(json!({
        "ok": true,
        "tenantId": tenant_id,
        "connector": reply.connector,
        "conversationId": reply.conversation_id,
        "inReplyTo": reply.in_reply_to,
        "bodyHash": sha256_hex(reply.text.as_bytes()),
        "delivery": "accepted"
    }))
    .into_response()
}

impl RelayState {
    fn channel_for(&self, tenant_id: &str) -> TenantChannel {
        self.tenants
            .entry(tenant_id.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(128);
                TenantChannel {
                    tx,
                    queue: Arc::new(Mutex::new(VecDeque::new())),
                }
            })
            .clone()
    }
}

impl TenantChannel {
    fn enqueue(&self, envelope: RelayEnvelope) {
        let mut queue = self.queue.lock().expect("tenant queue mutex poisoned");
        queue.push_back(envelope);
        while queue.len() > 100 {
            queue.pop_front();
        }
    }

    fn snapshot_queue(&self) -> Vec<RelayEnvelope> {
        let queue = self.queue.lock().expect("tenant queue mutex poisoned");
        queue.iter().cloned().collect()
    }
}

fn challenge_response(payload: &Value) -> Option<String> {
    payload
        .get("challenge")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .get("event")
                .and_then(|event| event.get("challenge"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
}

fn verify_shared_secret(state: &RelayState, headers: &HeaderMap, body: impl AsRef<[u8]>) -> bool {
    let Some(secret) = &state.shared_secret else {
        return true;
    };
    let Some(signature) = headers
        .get("x-multica-signature")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    hmac_sha256_hex(secret.as_bytes(), body.as_ref()) == signature
}

fn hmac_sha256_hex(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

fn sha256_hex(body: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_ref());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_platform_challenge() {
        assert_eq!(
            challenge_response(&json!({"challenge": "abc"})).as_deref(),
            Some("abc")
        );
        assert_eq!(
            challenge_response(&json!({"event": {"challenge": "nested"}})).as_deref(),
            Some("nested")
        );
    }

    #[test]
    fn computes_expected_hmac() {
        assert_eq!(
            hmac_sha256_hex(b"secret", br#"{"ok":true}"#),
            "f6b4a2841c93f8bf2fb8f2c13d8fb0b6c8e8019f09ee405d248daa8385fad638"
        );
    }
}
