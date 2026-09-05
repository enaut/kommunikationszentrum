use crate::models::domain::Domain;
use serde::{Deserialize, Serialize};
use spacetimedb::SpacetimeType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, SpacetimeType)]
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
