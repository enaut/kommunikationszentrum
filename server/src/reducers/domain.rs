use log::{info, warn};
use spacetimedb::{ProcedureContext, Table};

use crate::common::auth::is_admin_identity;
use crate::models::domain::*;
use crate::services::stalwart::client::send_stalwart_jmap_request;
use crate::services::stalwart::domain::*;

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

    // 2) Build single JMAP request with x:Domain/query and referenced x:Domain/get
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

    // 3) Perform HTTP request via unified Stalwart JMAP client
    let res_body = send_stalwart_jmap_request(ctx, payload)?;

    // 4) Parse JMAP response
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
