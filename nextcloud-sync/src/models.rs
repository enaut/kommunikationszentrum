use serde::de::{self, Deserializer};
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

/// Raw representation from Nextcloud OCS API (/ocs/v2.php/cloud/users/{uid} or /users/details)
#[derive(Debug, Clone)]
pub struct OcsUserDetails {
    pub id: String,
    pub displayname: Option<String>,
    pub email: Option<String>,
    pub notify_email: Option<String>,
    pub additional_mail: Option<Vec<String>>,
    pub enabled: bool,
}

impl OcsUserDetails {
    pub fn from_json_value(val: serde_json::Value) -> Result<Self, String> {
        let mut map = match val {
            serde_json::Value::Object(m) => m,
            other => return Err(format!("Expected user object, got {other:?}")),
        };

        let id = match map.remove("id") {
            Some(serde_json::Value::String(s)) => s,
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };

        let displayname = map
            .remove("displayname")
            .or_else(|| map.remove("display-name"))
            .or_else(|| map.remove("displayName"))
            .and_then(parse_string_opt);

        let email = map
            .remove("email")
            .or_else(|| map.remove("mail"))
            .and_then(parse_string_opt);

        let notify_email = map
            .remove("notify_email")
            .or_else(|| map.remove("notify-email"))
            .or_else(|| map.remove("notifyEmail"))
            .and_then(parse_string_opt);

        let additional_mail = map
            .remove("additional_mail")
            .or_else(|| map.remove("additional-mail"))
            .or_else(|| map.remove("additionalMail"))
            .and_then(|v| match v {
                serde_json::Value::Array(arr) => {
                    let list: Vec<String> = arr
                        .into_iter()
                        .filter_map(|item| match item {
                            serde_json::Value::String(s) => {
                                let t = s.trim();
                                if t.is_empty() {
                                    None
                                } else {
                                    Some(t.to_string())
                                }
                            }
                            serde_json::Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .collect();
                    Some(list)
                }
                _ => None,
            });

        let enabled = map.remove("enabled").map(parse_bool).unwrap_or(true);

        Ok(Self {
            id,
            displayname,
            email,
            notify_email,
            additional_mail,
            enabled,
        })
    }
}

impl<'de> Deserialize<'de> for OcsUserDetails {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = serde_json::Value::deserialize(deserializer)?;
        Self::from_json_value(val).map_err(de::Error::custom)
    }
}

fn parse_string_opt(v: serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_bool(v: serde_json::Value) -> bool {
    match v {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_i64().map(|i| i != 0).unwrap_or(true),
        serde_json::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "false" | "0" | "no" => false,
            "true" | "1" | "yes" => true,
            _ => true,
        },
        _ => true,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct OcsResponse<T> {
    pub ocs: OcsDataWrapper<T>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct OcsDataWrapper<T> {
    pub meta: OcsMeta,
    #[serde(default, deserialize_with = "deserialize_nullable_or_empty_array")]
    pub data: Option<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OcsMeta {
    #[allow(dead_code)]
    #[serde(default)]
    pub status: String,
    #[serde(default, deserialize_with = "deserialize_flexible_statuscode")]
    pub statuscode: i64,
    #[serde(default)]
    pub message: Option<String>,
}

/// Wrapper for the data object returned by /ocs/v2.php/cloud/users/details
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsersWrapper {
    #[serde(default, deserialize_with = "deserialize_users_field")]
    pub users: Vec<OcsUserDetails>,
}

fn deserialize_flexible_statuscode<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| de::Error::custom("Invalid integer statuscode")),
        serde_json::Value::String(s) => s.trim().parse::<i64>().map_err(de::Error::custom),
        other => Err(de::Error::custom(format!(
            "Expected integer or string for statuscode, got {other:?}"
        ))),
    }
}

pub fn deserialize_nullable_or_empty_array<'de, D, T>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Array(ref arr) if arr.is_empty() => Ok(None),
        other => T::deserialize(other).map(Some).map_err(de::Error::custom),
    }
}

pub fn deserialize_users_field<'de, D>(deserializer: D) -> Result<Vec<OcsUserDetails>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::Array(arr) => {
            let mut users = Vec::with_capacity(arr.len());
            for item in arr {
                let user: OcsUserDetails =
                    OcsUserDetails::deserialize(item).map_err(de::Error::custom)?;
                users.push(user);
            }
            Ok(users)
        }
        serde_json::Value::Object(map) => {
            let mut users = Vec::with_capacity(map.len());
            for (key, item) in map {
                let mut user: OcsUserDetails =
                    OcsUserDetails::deserialize(item).map_err(de::Error::custom)?;
                if user.id.is_empty() {
                    user.id = key;
                }
                users.push(user);
            }
            Ok(users)
        }
        other => Err(de::Error::custom(format!(
            "Expected array or object for 'users', got {other:?}"
        ))),
    }
}

pub fn map_ocs_user_to_upsert(user: OcsUserDetails) -> Option<UserSyncRequest> {
    let email = user
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            user.notify_email
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            user.additional_mail
                .as_ref()
                .and_then(|list| list.iter().map(|s| s.trim()).find(|s| !s.is_empty()))
        });

    let Some(raw_email) = email else {
        tracing::warn!(uid = %user.id, "Nextcloud user has no email; skipping sync");
        return None;
    };

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
            notify_email: None,
            additional_mail: None,
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
    fn test_map_ocs_user_fallback_email() {
        let ocs_user = OcsUserDetails {
            id: "bob".to_string(),
            displayname: Some("Bob".to_string()),
            email: None,
            notify_email: Some("bob_notify@example.com".to_string()),
            additional_mail: Some(vec!["bob_add@example.com".to_string()]),
            enabled: true,
        };
        let req = map_ocs_user_to_upsert(ocs_user).expect("Should resolve notify_email");
        assert_eq!(req.user.emails[0].email, "bob_notify@example.com");

        let ocs_user_additional = OcsUserDetails {
            id: "charlie".to_string(),
            displayname: Some("Charlie".to_string()),
            email: None,
            notify_email: None,
            additional_mail: Some(vec!["charlie@example.com".to_string()]),
            enabled: true,
        };
        let req2 =
            map_ocs_user_to_upsert(ocs_user_additional).expect("Should resolve additional_mail");
        assert_eq!(req2.user.emails[0].email, "charlie@example.com");
    }

    #[test]
    fn test_map_ocs_user_without_email_skipped() {
        let ocs_user = OcsUserDetails {
            id: "bob".to_string(),
            displayname: Some("Bob".to_string()),
            email: None,
            notify_email: None,
            additional_mail: None,
            enabled: true,
        };
        assert!(map_ocs_user_to_upsert(ocs_user).is_none());

        let ocs_user_empty_email = OcsUserDetails {
            id: "charlie".to_string(),
            displayname: Some("Charlie".to_string()),
            email: Some("   ".to_string()),
            notify_email: None,
            additional_mail: None,
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

    #[test]
    fn test_deserialize_nextcloud_with_duplicate_displayname_fields() {
        // Nextcloud outputs both "displayname" AND "display-name" in the same object.
        let json = r#"{
            "ocs": {
                "meta": {
                    "status": "ok",
                    "statuscode": 200,
                    "message": "OK"
                },
                "data": {
                    "users": {
                        "2012puppa@googlemail.com": {
                            "enabled": false,
                            "storageLocation": "/var/www/nextcloud/data/2012puppa@googlemail.com",
                            "id": "2012puppa@googlemail.com",
                            "email": "2012puppa@googlemail.com",
                            "additional_mail": [],
                            "displayname": "Yannic Hils",
                            "display-name": "Yannic Hils",
                            "notify_email": null
                        }
                    }
                }
            }
        }"#;

        let res: OcsResponse<UsersWrapper> = serde_json::from_str(json).unwrap();
        assert_eq!(res.ocs.meta.statuscode, 200);
        let users = res.ocs.data.unwrap().users;
        assert_eq!(users.len(), 1);
        let u = &users[0];
        assert_eq!(u.id, "2012puppa@googlemail.com");
        assert_eq!(u.displayname.as_deref(), Some("Yannic Hils"));
        assert_eq!(u.email.as_deref(), Some("2012puppa@googlemail.com"));
        assert!(!u.enabled);
    }

    #[test]
    fn test_deserialize_ocs_response_users_as_map() {
        let json = r#"{
            "ocs": {
                "meta": {
                    "status": "ok",
                    "statuscode": 200,
                    "message": "OK"
                },
                "data": {
                    "users": {
                        "alice": {
                            "id": "alice",
                            "displayname": "Alice W",
                            "email": "alice@example.com",
                            "enabled": true
                        },
                        "bob": {
                            "id": "bob",
                            "display-name": "Bob B",
                            "mail": "bob@example.com",
                            "enabled": "true"
                        }
                    },
                    "groups": []
                }
            }
        }"#;

        let res: OcsResponse<UsersWrapper> = serde_json::from_str(json).unwrap();
        assert_eq!(res.ocs.meta.statuscode, 200);
        let users = res.ocs.data.unwrap().users;
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn test_deserialize_ocs_response_users_as_list() {
        let json = r#"{
            "ocs": {
                "meta": {
                    "status": "ok",
                    "statuscode": 200,
                    "message": "OK"
                },
                "data": {
                    "users": [
                        {
                            "id": 1001,
                            "displayName": "User 1001",
                            "notify_email": "u1001@example.com",
                            "enabled": 1
                        }
                    ]
                }
            }
        }"#;

        let res: OcsResponse<UsersWrapper> = serde_json::from_str(json).unwrap();
        assert_eq!(res.ocs.meta.statuscode, 200);
        let users = res.ocs.data.unwrap().users;
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].id, "1001");
        assert_eq!(users[0].displayname.as_deref(), Some("User 1001"));
        assert_eq!(users[0].notify_email.as_deref(), Some("u1001@example.com"));
        assert!(users[0].enabled);
    }

    #[test]
    fn test_deserialize_ocs_response_empty_users_array() {
        let json = r#"{
            "ocs": {
                "meta": {
                    "status": "ok",
                    "statuscode": "200",
                    "message": "OK"
                },
                "data": {
                    "users": [],
                    "groups": []
                }
            }
        }"#;

        let res: OcsResponse<UsersWrapper> = serde_json::from_str(json).unwrap();
        assert_eq!(res.ocs.meta.statuscode, 200);
        let users = res.ocs.data.unwrap().users;
        assert!(users.is_empty());
    }

    #[test]
    fn test_deserialize_ocs_response_data_empty_array_error() {
        let json = r#"{
            "ocs": {
                "meta": {
                    "status": "failure",
                    "statuscode": 997,
                    "message": "Current user is not logged in"
                },
                "data": []
            }
        }"#;

        let res: OcsResponse<UsersWrapper> = serde_json::from_str(json).unwrap();
        assert_eq!(res.ocs.meta.statuscode, 997);
        assert_eq!(
            res.ocs.meta.message.as_deref(),
            Some("Current user is not logged in")
        );
        assert!(res.ocs.data.is_none());

        let res_single: OcsResponse<OcsUserDetails> = serde_json::from_str(json).unwrap();
        assert_eq!(res_single.ocs.meta.statuscode, 997);
        assert!(res_single.ocs.data.is_none());
    }

    #[test]
    fn test_deserialize_single_user_details() {
        let json = r#"{
            "ocs": {
                "meta": {
                    "status": "ok",
                    "statuscode": 100,
                    "message": "OK"
                },
                "data": {
                    "id": "admin",
                    "displayname": "Administrator",
                    "email": "admin@example.org",
                    "enabled": true
                }
            }
        }"#;

        let res: OcsResponse<OcsUserDetails> = serde_json::from_str(json).unwrap();
        let user = res.ocs.data.unwrap();
        assert_eq!(user.id, "admin");
        assert_eq!(user.displayname.as_deref(), Some("Administrator"));
        assert_eq!(user.email.as_deref(), Some("admin@example.org"));
        assert!(user.enabled);
    }
}
