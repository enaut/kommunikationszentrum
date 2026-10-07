use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedEmail {
    pub email: String,
    pub is_primary: bool,
    #[serde(default)]
    pub is_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSyncPayload {
    pub external_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_admin: Option<bool>,
    pub emails: Vec<SyncedEmail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSyncRequest {
    pub action: String, // "upsert" or "delete"
    pub user: UserSyncPayload,
}

/// Raw representation from Nextcloud OCS API (/ocs/v2.php/cloud/users/{uid})
#[derive(Debug, Clone, Deserialize)]
pub struct OcsUserDetails {
    pub id: String,
    #[serde(default)]
    pub displayname: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct OcsResponse<T> {
    pub ocs: OcsDataWrapper<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OcsDataWrapper<T> {
    pub meta: OcsMeta,
    pub data: T,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OcsMeta {
    #[allow(dead_code)]
    pub status: String,
    pub statuscode: i64,
    #[serde(default)]
    pub message: Option<String>,
}

pub fn map_ocs_user_to_upsert(user: OcsUserDetails) -> Option<UserSyncRequest> {
    let raw_email = user.email.as_deref().unwrap_or("").trim();
    if raw_email.is_empty() {
        tracing::warn!(uid = %user.id, "Nextcloud user has no email; skipping sync");
        return None;
    }

    Some(UserSyncRequest {
        action: "upsert".to_string(),
        user: UserSyncPayload {
            external_id: user.id,
            name: user.displayname.filter(|s| !s.trim().is_empty()),
            is_active: Some(user.enabled),
            is_admin: None, // Keep existing admin role untouched in Kommunikationszentrum
            emails: vec![SyncedEmail {
                email: raw_email.to_string(),
                is_primary: true,
                is_verified: true, // Nextcloud accounts are considered verified
            }],
        },
    })
}

pub fn map_uid_to_delete(uid: &str) -> UserSyncRequest {
    UserSyncRequest {
        action: "delete".to_string(),
        user: UserSyncPayload {
            external_id: uid.to_string(),
            name: None,
            is_active: None,
            is_admin: None,
            emails: vec![],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_ocs_user_to_upsert_success() {
        let ocs_user = OcsUserDetails {
            id: "alice".to_string(),
            displayname: Some("Alice Wonderland".to_string()),
            email: Some("alice@example.com".to_string()),
            enabled: true,
        };

        let req = map_ocs_user_to_upsert(ocs_user).expect("Should map successfully");
        assert_eq!(req.action, "upsert");
        assert_eq!(req.user.external_id, "alice");
        assert_eq!(req.user.name.as_deref(), Some("Alice Wonderland"));
        assert_eq!(req.user.is_active, Some(true));
        assert_eq!(req.user.is_admin, None);
        assert_eq!(req.user.emails.len(), 1);
        assert_eq!(req.user.emails[0].email, "alice@example.com");
        assert!(req.user.emails[0].is_primary);
        assert!(req.user.emails[0].is_verified);
    }

    #[test]
    fn test_map_ocs_user_without_email_skipped() {
        let ocs_user = OcsUserDetails {
            id: "bob".to_string(),
            displayname: Some("Bob".to_string()),
            email: None,
            enabled: true,
        };
        assert!(map_ocs_user_to_upsert(ocs_user).is_none());

        let ocs_user_empty_email = OcsUserDetails {
            id: "charlie".to_string(),
            displayname: Some("Charlie".to_string()),
            email: Some("   ".to_string()),
            enabled: true,
        };
        assert!(map_ocs_user_to_upsert(ocs_user_empty_email).is_none());
    }

    #[test]
    fn test_map_uid_to_delete() {
        let req = map_uid_to_delete("alice");
        assert_eq!(req.action, "delete");
        assert_eq!(req.user.external_id, "alice");
    }
}
