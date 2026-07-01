use crate::store::Store;
use crate::types::{
    ConnectorMessage, PermissionDecision, PermissionKind, PermissionRequest, PermissionStatus,
};
use anyhow::Result;
use chrono::Utc;
use serde_json::json;

#[derive(Clone)]
pub struct PermissionBroker {
    store: Store,
}

impl PermissionBroker {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn request_connector_task(&self, message: &ConnectorMessage) -> Result<PermissionRequest> {
        let mut request = PermissionRequest::pending(
            PermissionKind::ConnectorReply,
            format!(
                "{} 用户 {} 请求启动智能体任务",
                message.connector, message.sender_id
            ),
            json!({
                "connector": message.connector,
                "tenantId": message.tenant_id,
                "conversationId": message.conversation_id,
                "senderId": message.sender_id,
                "preview": message.text.chars().take(240).collect::<String>()
            }),
        );
        request.connector_message_id = Some(message.id.clone());
        self.store.insert_permission_request(&request)?;
        self.store
            .record_audit_event("permission.requested", Some(&request.id), &request)?;
        Ok(request)
    }

    pub fn decide(&self, request_id: String, approved: bool, decided_by: String) -> Result<()> {
        let decision = PermissionDecision {
            request_id,
            approved,
            decided_by,
            reason: None,
            decided_at: Utc::now(),
        };
        self.store.record_permission_decision(&decision)
    }

    pub fn pending(&self) -> Result<Vec<PermissionRequest>> {
        self.store.list_pending_permissions()
    }

    pub fn expire_stale(&self) -> Result<Vec<PermissionRequest>> {
        let stale = self
            .store
            .list_pending_permissions()?
            .into_iter()
            .filter(|request| Utc::now() - request.created_at > chrono::Duration::hours(24))
            .map(|mut request| {
                request.status = PermissionStatus::Expired;
                request
            })
            .collect();
        Ok(stale)
    }
}
