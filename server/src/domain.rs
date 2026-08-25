use crate::account::{is_admin_identity, is_admin_user};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use spacetimedb::{ProcedureContext, Query, SpacetimeType, Table, ViewContext};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[spacetimedb::table(accessor = domains)]
pub struct Domain {
    /// Stalwart domain ID (stable external identifier)
    #[primary_key]
    pub id: String,
    /// Domain name (e.g. "example.com")
    #[unique]
    pub name: String,
    /// Optional description from Stalwart
    pub description: Option<String>,
}

#[spacetimedb::view(accessor = visible_domains, public)]
pub fn visible_domains(ctx: &ViewContext) -> impl Query<Domain> {
    let is_admin = is_admin_user(ctx);
    ctx.from.domains().r#filter(move |_| is_admin)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SpacetimeType)]
pub struct SyncDomainsResult {
    pub domains_found: u32,
    pub domains_added: u32,
    pub domains_updated: u32,
    pub domains_removed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StalwartDomainItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainSyncAction {
    Insert(Domain),
    Update(Domain),
    Delete(String),
}

/// Pure domain synchronization diff calculation.
/// Identifies additions, updates, and removals to ensure local DB matches Stalwart.
pub fn calculate_domain_sync(
    current_domains: &[Domain],
    incoming_domains: &[StalwartDomainItem],
) -> Result<(Vec<DomainSyncAction>, SyncDomainsResult), String> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    for inc in incoming_domains {
        if inc.id.trim().is_empty() {
            return Err("Invalid domain: empty domain ID".to_string());
        }
        if inc.name.trim().is_empty() {
            return Err(format!("Invalid domain '{}': empty domain name", inc.id));
        }
        if !seen_ids.insert(&inc.id) {
            return Err(format!(
                "Duplicate domain ID '{}' in Stalwart response",
                inc.id
            ));
        }
        if !seen_names.insert(&inc.name) {
            return Err(format!(
                "Duplicate domain name '{}' in Stalwart response",
                inc.name
            ));
        }
    }

    let mut actions = Vec::new();
    let mut domains_added = 0u32;
    let mut domains_updated = 0u32;
    let mut domains_removed = 0u32;

    let incoming_id_set: std::collections::HashSet<&str> =
        incoming_domains.iter().map(|d| d.id.as_str()).collect();

    // 1. Identify removals (local domains that no longer exist in Stalwart)
    for current in current_domains {
        if !incoming_id_set.contains(current.id.as_str()) {
            actions.push(DomainSyncAction::Delete(current.id.clone()));
            domains_removed += 1;
        }
    }

    // Map current domains by ID for lookup
    let current_by_id: std::collections::HashMap<&str, &Domain> =
        current_domains.iter().map(|d| (d.id.as_str(), d)).collect();

    // 2. Identify additions and updates
    for inc in incoming_domains {
        let domain_obj = Domain {
            id: inc.id.clone(),
            name: inc.name.clone(),
            description: inc.description.clone(),
        };

        if let Some(existing) = current_by_id.get(inc.id.as_str()) {
            if existing.name != inc.name || existing.description != inc.description {
                actions.push(DomainSyncAction::Update(domain_obj));
                domains_updated += 1;
            }
        } else {
            actions.push(DomainSyncAction::Insert(domain_obj));
            domains_added += 1;
        }
    }

    let result = SyncDomainsResult {
        domains_found: incoming_domains.len() as u32,
        domains_added,
        domains_updated,
        domains_removed,
    };

    Ok((actions, result))
}

/// Parses the JMAP response JSON from Stalwart `x:Domain/query` and `x:Domain/get`.
pub fn parse_jmap_domain_response(
    res_body: &serde_json::Value,
) -> Result<Vec<StalwartDomainItem>, String> {
    let method_responses = res_body
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid JMAP response: missing 'methodResponses' array".to_string())?;

    // Check for any top-level method errors
    for entry in method_responses {
        if let Some(method_name) = entry.get(0).and_then(|v| v.as_str()) {
            if method_name == "error" {
                let err_obj = entry.get(1);
                let err_type = err_obj
                    .and_then(|o| o.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let err_desc = err_obj
                    .and_then(|o| o.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                return Err(format!("JMAP method error ({}): {}", err_type, err_desc));
            }
        }
    }

    // Look for x:Domain/query
    let query_entry = method_responses
        .iter()
        .find(|entry| entry.get(0).and_then(|v| v.as_str()) == Some("x:Domain/query"));

    let query_args = match query_entry {
        Some(entry) => entry.get(1).ok_or_else(|| {
            "Invalid JMAP response: missing arguments in 'x:Domain/query'".to_string()
        })?,
        None => {
            return Err(
                "Invalid JMAP response: missing 'x:Domain/query' methodResponse".to_string(),
            )
        }
    };

    let ids_array = query_args
        .get("ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "Invalid JMAP response: missing or non-array '/ids' in 'x:Domain/query'".to_string()
        })?;

    if ids_array.is_empty() {
        return Ok(Vec::new());
    }

    // Look for x:Domain/get
    let get_entry = method_responses
        .iter()
        .find(|entry| entry.get(0).and_then(|v| v.as_str()) == Some("x:Domain/get"));

    let get_args = match get_entry {
        Some(entry) => entry.get(1).ok_or_else(|| {
            "Invalid JMAP response: missing arguments in 'x:Domain/get'".to_string()
        })?,
        None => {
            return Err("Invalid JMAP response: missing 'x:Domain/get' methodResponse".to_string())
        }
    };

    let list_array = get_args
        .get("list")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "Invalid JMAP response: missing or non-array '/list' in 'x:Domain/get'".to_string()
        })?;

    let mut items = Vec::with_capacity(list_array.len());
    for item in list_array {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Invalid domain object: missing 'id'".to_string())?
            .trim()
            .to_string();

        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Invalid domain object: missing 'name'".to_string())?
            .trim()
            .to_string();

        if id.is_empty() || name.is_empty() {
            return Err("Invalid domain object: 'id' and 'name' must not be empty".to_string());
        }

        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        items.push(StalwartDomainItem {
            id,
            name,
            description,
        });
    }

    // Validate duplicate IDs and names
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    for item in &items {
        if !seen_ids.insert(&item.id) {
            return Err(format!(
                "Duplicate domain ID '{}' in Stalwart response",
                item.id
            ));
        }
        if !seen_names.insert(&item.name) {
            return Err(format!(
                "Duplicate domain name '{}' in Stalwart response",
                item.name
            ));
        }
    }

    Ok(items)
}

/// SpacetimeDB Procedure: Synchronizes domains configured in Stalwart into the SpacetimeDB database.
/// Only callable by module owner/admins.
#[spacetimedb::procedure]
pub fn sync_stalwart_domains(ctx: &mut ProcedureContext) -> Result<SyncDomainsResult, String> {
    info!("Executing sync_stalwart_domains procedure");

    // 1) Authorization check: only owner/admin identity
    let caller = ctx.sender();
    let is_admin: bool = ctx.with_tx(|tx| is_admin_identity(tx, caller));
    if !is_admin {
        warn!("Unauthorized sync_stalwart_domains call from {:?}", caller);
        return Err(format!(
            "Unauthorized: caller {:?} is not a module owner/admin",
            caller
        ));
    }

    // 2) Read compile-time credentials
    let jmap_base = env!("STALWART_JMAP_URL");
    let admin_token = env!("STALWART_ADMIN_TOKEN");

    let endpoint = if jmap_base.ends_with("/jmap") {
        jmap_base.trim_end_matches('/').to_string()
    } else {
        format!("{}/jmap", jmap_base.trim_end_matches('/'))
    };

    // 3) Build single JMAP request with x:Domain/query and referenced x:Domain/get
    let payload = serde_json::json!({
        "using": [
            "urn:ietf:params:jmap:core",
            "urn:stalwart:jmap"
        ],
        "methodCalls": [
            [
                "x:Domain/query",
                {},
                "q"
            ],
            [
                "x:Domain/get",
                {
                    "#ids": {
                        "resultOf": "q",
                        "name": "x:Domain/query",
                        "path": "/ids"
                    }
                },
                "g"
            ]
        ]
    });

    let body_bytes = serde_json::to_vec(&payload)
        .map_err(|e| format!("Failed to serialize JMAP payload: {}", e))?;

    let request = spacetimedb::http::Request::builder()
        .uri(endpoint)
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_token))
        .extension(spacetimedb::http::Timeout(
            spacetimedb::TimeDuration::from_micros(30_000_000), // 30s timeout
        ))
        .body(body_bytes)
        .map_err(|e| format!("Failed to build HTTP request: {:?}", e))?;

    // 4) Perform HTTP request
    let response = ctx.http.send(request).map_err(|e| {
        error!("Failed to perform Stalwart HTTP request: {:?}", e);
        format!("HTTP request to Stalwart failed: {:?}", e)
    })?;

    let (parts, body) = response.into_parts();

    if parts.status == 401 || parts.status == 403 {
        error!("Stalwart authentication failed: HTTP {}", parts.status);
        return Err(format!(
            "Stalwart authentication failed (HTTP {})",
            parts.status
        ));
    }

    if parts.status != 200 {
        let body_str = body.into_string_lossy();
        error!(
            "Stalwart returned non-200 HTTP status {}: {}",
            parts.status, body_str
        );
        return Err(format!(
            "Stalwart returned HTTP status {}: {}",
            parts.status, body_str
        ));
    }

    // 5) Parse JMAP response
    let body_bytes = body.into_bytes();
    let res_body: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| format!("Failed to parse Stalwart JMAP response JSON: {}", e))?;

    let stalwart_domains = parse_jmap_domain_response(&res_body)?;

    // 6) Transactional database update
    let result = ctx.with_tx(|tx| {
        let current_domains: Vec<Domain> = tx.db.domains().iter().collect();
        let (actions, result) = calculate_domain_sync(&current_domains, &stalwart_domains)?;

        for action in actions {
            match action {
                DomainSyncAction::Delete(id) => {
                    tx.db.domains().id().delete(&id);
                }
                DomainSyncAction::Update(domain) => {
                    tx.db.domains().id().update(domain);
                }
                DomainSyncAction::Insert(domain) => {
                    tx.db.domains().insert(domain);
                }
            }
        }

        Ok::<SyncDomainsResult, String>(result)
    })?;

    info!(
        "Stalwart domain sync completed: found={}, added={}, updated={}, removed={}",
        result.domains_found, result.domains_added, result.domains_updated, result.domains_removed
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_stalwart_domain_list() {
        let current = vec![];
        let incoming = vec![];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 0);
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 0,
                domains_added: 0,
                domains_updated: 0,
                domains_removed: 0,
            }
        );
    }

    #[test]
    fn test_empty_stalwart_removes_existing_local_domains() {
        let current = vec![
            Domain {
                id: "d1".into(),
                name: "example.com".into(),
                description: None,
            },
            Domain {
                id: "d2".into(),
                name: "example.org".into(),
                description: Some("Test".into()),
            },
        ];
        let incoming = vec![];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], DomainSyncAction::Delete("d1".into()));
        assert_eq!(actions[1], DomainSyncAction::Delete("d2".into()));
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 0,
                domains_added: 0,
                domains_updated: 0,
                domains_removed: 2,
            }
        );
    }

    #[test]
    fn test_adding_new_domain() {
        let current = vec![];
        let incoming = vec![StalwartDomainItem {
            id: "d1".into(),
            name: "example.com".into(),
            description: Some("Primary Domain".into()),
        }];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            DomainSyncAction::Insert(Domain {
                id: "d1".into(),
                name: "example.com".into(),
                description: Some("Primary Domain".into()),
            })
        );
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 1,
                domains_added: 1,
                domains_updated: 0,
                domains_removed: 0,
            }
        );
    }

    #[test]
    fn test_updating_existing_domain() {
        let current = vec![Domain {
            id: "d1".into(),
            name: "old-name.example".into(),
            description: None,
        }];
        let incoming = vec![StalwartDomainItem {
            id: "d1".into(),
            name: "new-name.example".into(),
            description: Some("Updated description".into()),
        }];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            DomainSyncAction::Update(Domain {
                id: "d1".into(),
                name: "new-name.example".into(),
                description: Some("Updated description".into()),
            })
        );
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 1,
                domains_added: 0,
                domains_updated: 1,
                domains_removed: 0,
            }
        );
    }

    #[test]
    fn test_removing_stale_stalwart_domain() {
        let current = vec![
            Domain {
                id: "d1".into(),
                name: "keep.example".into(),
                description: None,
            },
            Domain {
                id: "d2".into(),
                name: "stale.example".into(),
                description: None,
            },
        ];
        let incoming = vec![StalwartDomainItem {
            id: "d1".into(),
            name: "keep.example".into(),
            description: None,
        }];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], DomainSyncAction::Delete("d2".into()));
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 1,
                domains_added: 0,
                domains_updated: 0,
                domains_removed: 1,
            }
        );
    }

    #[test]
    fn test_no_changes_when_local_state_matches_stalwart() {
        let current = vec![
            Domain {
                id: "d1".into(),
                name: "example.com".into(),
                description: Some("Primary".into()),
            },
            Domain {
                id: "d2".into(),
                name: "example.org".into(),
                description: None,
            },
        ];
        let incoming = vec![
            StalwartDomainItem {
                id: "d1".into(),
                name: "example.com".into(),
                description: Some("Primary".into()),
            },
            StalwartDomainItem {
                id: "d2".into(),
                name: "example.org".into(),
                description: None,
            },
        ];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 0);
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 2,
                domains_added: 0,
                domains_updated: 0,
                domains_removed: 0,
            }
        );
    }

    #[test]
    fn test_multiple_domains_mixed_operations() {
        let current = vec![
            Domain {
                id: "d1".into(),
                name: "unchanged.example".into(),
                description: None,
            },
            Domain {
                id: "d2".into(),
                name: "update-me.example".into(),
                description: None,
            },
            Domain {
                id: "d3".into(),
                name: "delete-me.example".into(),
                description: None,
            },
        ];
        let incoming = vec![
            StalwartDomainItem {
                id: "d1".into(),
                name: "unchanged.example".into(),
                description: None,
            },
            StalwartDomainItem {
                id: "d2".into(),
                name: "update-me.example".into(),
                description: Some("New description".into()),
            },
            StalwartDomainItem {
                id: "d4".into(),
                name: "new.example".into(),
                description: Some("Brand new".into()),
            },
        ];
        let (actions, result) = calculate_domain_sync(&current, &incoming).unwrap();

        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&DomainSyncAction::Delete("d3".into())));
        assert!(actions.contains(&DomainSyncAction::Update(Domain {
            id: "d2".into(),
            name: "update-me.example".into(),
            description: Some("New description".into()),
        })));
        assert!(actions.contains(&DomainSyncAction::Insert(Domain {
            id: "d4".into(),
            name: "new.example".into(),
            description: Some("Brand new".into()),
        })));
        assert_eq!(
            result,
            SyncDomainsResult {
                domains_found: 3,
                domains_added: 1,
                domains_updated: 1,
                domains_removed: 1,
            }
        );
    }

    #[test]
    fn test_duplicate_and_invalid_input_handling() {
        // Duplicate ID
        let current = vec![];
        let duplicate_id = vec![
            StalwartDomainItem {
                id: "d1".into(),
                name: "example.com".into(),
                description: None,
            },
            StalwartDomainItem {
                id: "d1".into(),
                name: "example.org".into(),
                description: None,
            },
        ];
        assert!(calculate_domain_sync(&current, &duplicate_id).is_err());

        // Duplicate name
        let duplicate_name = vec![
            StalwartDomainItem {
                id: "d1".into(),
                name: "example.com".into(),
                description: None,
            },
            StalwartDomainItem {
                id: "d2".into(),
                name: "example.com".into(),
                description: None,
            },
        ];
        assert!(calculate_domain_sync(&current, &duplicate_name).is_err());

        // Empty ID
        let empty_id = vec![StalwartDomainItem {
            id: "".into(),
            name: "example.com".into(),
            description: None,
        }];
        assert!(calculate_domain_sync(&current, &empty_id).is_err());

        // Empty name
        let empty_name = vec![StalwartDomainItem {
            id: "d1".into(),
            name: "   ".into(),
            description: None,
        }];
        assert!(calculate_domain_sync(&current, &empty_name).is_err());
    }

    #[test]
    fn test_parse_jmap_domain_response_success() {
        let json_val = serde_json::json!({
            "sessionState": "abc",
            "methodResponses": [
                [
                    "x:Domain/query",
                    {
                        "ids": ["d1", "d2"],
                        "total": 2,
                        "position": 0
                    },
                    "q"
                ],
                [
                    "x:Domain/get",
                    {
                        "list": [
                            {
                                "id": "d1",
                                "name": "example.com",
                                "description": "Primary domain"
                            },
                            {
                                "id": "d2",
                                "name": "example.org"
                            }
                        ],
                        "notFound": []
                    },
                    "g"
                ]
            ]
        });

        let items = parse_jmap_domain_response(&json_val).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0],
            StalwartDomainItem {
                id: "d1".into(),
                name: "example.com".into(),
                description: Some("Primary domain".into()),
            }
        );
        assert_eq!(
            items[1],
            StalwartDomainItem {
                id: "d2".into(),
                name: "example.org".into(),
                description: None,
            }
        );
    }

    #[test]
    fn test_parse_jmap_domain_response_empty_query() {
        let json_val = serde_json::json!({
            "methodResponses": [
                [
                    "x:Domain/query",
                    {
                        "ids": [],
                        "total": 0
                    },
                    "q"
                ],
                [
                    "x:Domain/get",
                    {
                        "list": [],
                        "notFound": []
                    },
                    "g"
                ]
            ]
        });

        let items = parse_jmap_domain_response(&json_val).unwrap();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_parse_jmap_domain_response_method_error() {
        let json_val = serde_json::json!({
            "methodResponses": [
                [
                    "error",
                    {
                        "type": "invalidArguments",
                        "description": "unknown capability"
                    },
                    "q"
                ]
            ]
        });

        let err = parse_jmap_domain_response(&json_val).unwrap_err();
        assert!(err.contains("JMAP method error (invalidArguments): unknown capability"));
    }

    #[test]
    fn test_parse_jmap_domain_response_missing_fields() {
        // Missing ids in query
        let no_ids = serde_json::json!({
            "methodResponses": [
                [
                    "x:Domain/query",
                    {},
                    "q"
                ]
            ]
        });
        assert!(parse_jmap_domain_response(&no_ids).is_err());

        // Missing list in get
        let no_list = serde_json::json!({
            "methodResponses": [
                [
                    "x:Domain/query",
                    { "ids": ["d1"] },
                    "q"
                ],
                [
                    "x:Domain/get",
                    {},
                    "g"
                ]
            ]
        });
        assert!(parse_jmap_domain_response(&no_list).is_err());
    }
}
