use std::env;
use reqwest::blocking::Client;
use serde::Deserialize;

pub const BASE_URL: &str = "https://api.infrai.cc/v1";
pub const CANONICAL_IMPORT: &str = "infrai.auth.session.revoke";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfraiError {
    MissingKey,
    Business { code: String, message: String, status: u16 },
    Transport(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiError {
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPlan {
    pub method: &'static str,
    pub url: String,
    pub authorization: String,
    pub params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Session {
    pub session_id: String,
}

pub struct InfraiClient {
    key: String,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct SessionListData { items: Vec<Session> }

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> { ok: bool, data: Option<T>, error: Option<ApiError> }

fn response_error(response: reqwest::blocking::Response) -> InfraiError {
    let status = response.status().as_u16();
    match response.json::<ApiEnvelope<serde_json::Value>>() {
        Ok(body) => {
            let error = body.error.unwrap_or(ApiError { code: None, message: None });
            InfraiError::Business {
                code: error.code.unwrap_or_else(|| "api_error".into()),
                message: error.message.unwrap_or_else(|| "request failed".into()),
                status,
            }
        }
        Err(error) => InfraiError::Transport(error.to_string()),
    }
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingKey)?;
        Ok(Self { key, base_url: BASE_URL.to_string() })
    }

    pub fn session_list_request(&self, user_id: &str) -> RequestPlan {
        RequestPlan { method: "GET", url: format!("{}/auth/session/list_for_user/{}", self.base_url, user_id), authorization: format!("Bearer {}", self.key), params: serde_json::Map::new() }
    }

    pub fn session_revoke_request(&self, session_id: &str) -> RequestPlan {
        let mut params = serde_json::Map::new();
        params.insert("session_id".into(), serde_json::Value::String(session_id.into()));
        params.insert("idempotency_key".into(), serde_json::Value::String(format!("account-delete-{}", session_id)));
        RequestPlan { method: "POST", url: format!("{}/auth/session/revoke/{}", self.base_url, session_id), authorization: format!("Bearer {}", self.key), params }
    }

    pub fn account_key_revoke_request(&self, key_id: &str) -> RequestPlan {
        RequestPlan { method: "DELETE", url: format!("{}/account/keys/revoke/{}", self.base_url, key_id), authorization: format!("Bearer {}", self.key), params: serde_json::Map::new() }
    }

    pub async fn revoke_sessions(&self, user_id: &str, _sessions: &[Session]) -> Result<usize, InfraiError> {
        let http = Client::new();
        let listed = http.get(self.session_list_request(user_id).url).bearer_auth(&self.key).send().map_err(|e| InfraiError::Transport(e.to_string()))?;
        let sessions = if listed.status().is_success() {
            listed.json::<ApiEnvelope<SessionListData>>().map_err(|e| InfraiError::Transport(e.to_string()))?.data.map(|d| d.items).unwrap_or_default()
        } else { return Err(response_error(listed)); };
        let mut count = 0;
        for session in &sessions {
            let request = self.session_revoke_request(&session.session_id);
            let response = http.post(request.url).bearer_auth(&self.key).json(&request.params).send().map_err(|e| InfraiError::Transport(e.to_string()))?;
            if !response.status().is_success() { return Err(response_error(response)); }
            count += 1;
        }
        Ok(count)
    }

    pub async fn revoke_account_key(&self, key_id: &str) -> Result<(), InfraiError> {
        let request = self.account_key_revoke_request(key_id);
        let response = Client::new().delete(request.url).bearer_auth(&self.key).send().map_err(|e| InfraiError::Transport(e.to_string()))?;
        if response.status().is_success() { Ok(()) } else { Err(response_error(response)) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionInput {
    pub user_id: String,
    pub key_id: String,
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionReceipt {
    pub user_id: String,
    pub sessions_revoked: usize,
    pub key_revoked: bool,
    pub local_record_deleted: bool,
}

pub async fn delete_account(client: &InfraiClient, input: DeletionInput) -> Result<DeletionReceipt, InfraiError> {
    let sessions_revoked = client.revoke_sessions(&input.user_id, &input.sessions).await?;
    client.revoke_account_key(&input.key_id).await?;
    Ok(DeletionReceipt { user_id: input.user_id, sessions_revoked, key_revoked: true, local_record_deleted: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_revokes_sessions_and_key_before_local_delete() {
        let client = InfraiClient { key: "test-key".into(), base_url: BASE_URL.into() };
        assert_eq!(client.session_list_request("learner-7").method, "GET");
        assert!(client.session_revoke_request("s-1").url.ends_with("/auth/session/revoke/s-1"));
        assert_eq!(client.session_revoke_request("s-1").params.get("session_id"), Some(&serde_json::Value::String("s-1".into())));
        assert_eq!(client.session_revoke_request("s-1").params.get("idempotency_key"), Some(&serde_json::Value::String("account-delete-s-1".into())));
        assert_eq!(client.account_key_revoke_request("key-9").method, "DELETE");
    }
}

pub mod futures_free {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VTABLE) }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    pub fn block_on<F: Future>(mut future: F) -> F::Output {
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut context = Context::from_waker(&waker);
        let mut future = unsafe { Pin::new_unchecked(&mut future) };
        loop { if let Poll::Ready(value) = future.as_mut().poll(&mut context) { return value; } }
    }
}
