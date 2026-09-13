use log::info;
use spacetimedb::Table;

use crate::models::category::{
    category_app_passwords, message_categories, CategoryAppPassword, CategoryVisibility,
    MessageCategory, SubscriptionPermission,
};
use crate::models::domain::domains;
use crate::reducers::domain::do_sync_stalwart_domains;
use crate::services::stalwart::client::{send_stalwart_jmap_request, StalwartContext};

pub fn jmap_check_not_created(result: &serde_json::Value, label: &str) -> Result<(), String> {
    if let Some(not_created) = result.get("notCreated") {
        if not_created
            .as_object()
            .map(|entries| !entries.is_empty())
            .unwrap_or(false)
        {
            return Err(format!(
                "JMAP {} reported notCreated: {}",
                label, not_created
            ));
        }
    }
    Ok(())
}

/// Extract an object id from a JMAP Foo/set `created` entry. Stalwart may return
/// either a plain string id or an object such as `{"id": "..."}`.
pub fn jmap_created_id(value: &serde_json::Value) -> Option<String> {
    if let Some(id) = value.as_str() {
        return Some(id.to_string());
    }
    value
        .get("id")
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
}

pub fn jmap_method_result_by_name<'a>(
    res_body: &'a serde_json::Value,
    method_name: &str,
) -> Result<&'a serde_json::Value, String> {
    let method_responses = res_body
        .get("methodResponses")
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("Missing methodResponses in JMAP response: {}", res_body))?;

    for entry in method_responses {
        if entry.get(0).and_then(|value| value.as_str()) == Some(method_name) {
            return entry.get(1).ok_or_else(|| {
                format!("Missing result object for {} in JMAP response", method_name)
            });
        }
    }

    Err(format!(
        "Method {} not found in JMAP response: {}",
        method_name, res_body
    ))
}

pub fn provision_stalwart_app_password(
    ctx: &mut impl StalwartContext,
    account_id: &str,
    description: &str,
) -> Result<(String, String), String> {
    let payload = serde_json::json!({
        "using": [
            "urn:ietf:params:jmap:core",
            "urn:stalwart:jmap"
        ],
        "methodCalls": [
            [
                "x:AppPassword/set",
                {
                    "accountId": account_id,
                    "create": {
                        "app-pw-1": {
                            "description": description,
                            "permissions": {
                                "@type": "Replace",
                                "permissions": {
                                    "emailSend": true,
                                    "authenticate": true
                                }
                            },
                            "allowedIps": {}
                        }
                    }
                },
                "call-id-app-pw"
            ]
        ]
    });

    let res_body = send_stalwart_jmap_request(ctx, payload)?;
    let result = jmap_method_result_by_name(&res_body, "x:AppPassword/set")?;
    jmap_check_not_created(result, "x:AppPassword/set")?;

    let created = result
        .get("created")
        .and_then(|created| created.get("app-pw-1"))
        .ok_or_else(|| {
            format!(
                "Missing created app password in JMAP response: {}",
                res_body
            )
        })?;

    let stalwart_id = created
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("Missing app password id in JMAP response: {}", created))?
        .to_string();
    let secret = created
        .get("secret")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("Missing app password secret in JMAP response: {}", created))?
        .to_string();

    Ok((stalwart_id, secret))
}

pub fn provision_stalwart_category_mailbox(
    ctx: &mut impl StalwartContext,
    name: &str,
    email_address: &str,
    description: &str,
    visibility: CategoryVisibility,
    default_permission: SubscriptionPermission,
) -> Result<u64, String> {
    info!(
        "Provisioning Stalwart mailbox for category: name='{}', email='{}'",
        name, email_address
    );

    // 1) Parse email address into base and domain name
    let (base, domain_name) = email_address
        .split_once('@')
        .ok_or_else(|| format!("Invalid email address '{}': missing '@'", email_address))?;
    let base = base.trim();
    let domain_name = domain_name.trim();

    if base.is_empty() || domain_name.is_empty() {
        return Err(format!(
            "Invalid email address '{}': empty base or domain",
            email_address
        ));
    }

    // 2) Look up domain ID in local domains table; if missing, sync from Stalwart
    let mut domain = ctx.with_tx(|tx| tx.db.domains().name().find(&domain_name.to_string()));
    if domain.is_none() {
        info!(
            "Domain '{}' not found in local table, syncing domains from Stalwart...",
            domain_name
        );
        let _ = do_sync_stalwart_domains(ctx);
        domain = ctx.with_tx(|tx| tx.db.domains().name().find(&domain_name.to_string()));
    }
    let domain = domain.ok_or_else(|| {
        format!("Domain '{}' not found in Stalwart domains", domain_name)
    })?;
    let domain_id = domain.id;

    // 3) Check if account already exists in Stalwart
    let query_payload = serde_json::json!({
        "using": [
            "urn:ietf:params:jmap:core",
            "urn:stalwart:jmap"
        ],
        "methodCalls": [
            [
                "x:Account/query",
                {
                    "filter": {
                        "name": base,
                        "domainId": domain_id
                    }
                },
                "q"
            ]
        ]
    });

    let res = send_stalwart_jmap_request(ctx, query_payload)?;
    let query_result = jmap_method_result_by_name(&res, "x:Account/query")?;
    let existing_account_id = query_result
        .get("ids")
        .and_then(|ids| ids.as_array())
        .and_then(|arr| arr.first())
        .and_then(|val| val.as_str())
        .map(|s| s.to_string());

    let account_id = match existing_account_id {
        Some(id) => {
            info!("Found existing Stalwart account '{}' with id '{}'", base, id);
            id
        }
        None => {
            // Create account via x:Account/set
            let create_map = serde_json::json!({
                "create": {
                    "create-1": {
                        "@type": "User",
                        "name": base,
                        "description": name.trim(),
                        "domainId": domain_id,
                        "roles": {
                            "@type": "User"
                        },
                        "permissions": {
                            "@type": "Inherit"
                        },
                        "aliases": {},
                        "memberGroupIds": {},
                        "quotas": {},
                        "credentials": {},
                        "encryptionAtRest": {
                            "@type": "Disabled"
                        }
                    }
                }
            });

            let account_payload = serde_json::json!({
                "using": [
                    "urn:ietf:params:jmap:core",
                    "urn:stalwart:jmap"
                ],
                "methodCalls": [
                    ["x:Account/set", create_map, "call-id-1"]
                ]
            });

            let account_res = send_stalwart_jmap_request(ctx, account_payload)?;
            let account_result = jmap_method_result_by_name(&account_res, "x:Account/set")?;

            if let Some(created) = account_result.get("created").and_then(|c| c.get("create-1")) {
                jmap_created_id(created).ok_or_else(|| {
                    format!("Missing created account id in JMAP response: {}", account_res)
                })?
            } else if let Some(not_created) = account_result.get("notCreated").and_then(|nc| nc.get("create-1")) {
                if let Some(id) = not_created.get("objectId").and_then(|o| o.get("id")).and_then(|id| id.as_str()) {
                    id.to_string()
                } else {
                    return Err(format!("JMAP x:Account/set reported notCreated: {}", not_created));
                }
            } else {
                return Err(format!("Invalid response from x:Account/set: {}", account_res));
            }
        }
    };

    // 4) Create an app password for SMTP submission from this category mailbox
    let app_password_description = format!("kommunikationszentrum sender ({email_address})");
    let (stalwart_id, secret) =
        provision_stalwart_app_password(ctx, &account_id, &app_password_description)?;

    // 5) Persist CategoryAppPassword and insert or update MessageCategory
    let category_id = ctx.with_tx(|tx| {
        let app_password = tx.db.category_app_passwords().insert(CategoryAppPassword {
            id: 0,
            secret: secret.clone(),
            stalwart_id: stalwart_id.clone(),
            created_at: tx.timestamp,
        });

        if let Some(existing) = tx
            .db
            .message_categories()
            .email_address()
            .find(&email_address.to_string())
        {
            let updated = MessageCategory {
                app_password_id: Some(app_password.id),
                ..existing
            };
            tx.db.message_categories().id().update(updated);
            info!(
                "Updated existing category {} ({}) with app_password_id {}",
                existing.id, email_address, app_password.id
            );
            existing.id
        } else {
            let inserted = tx.db.message_categories().insert(MessageCategory {
                id: 0,
                name: name.to_string(),
                email_address: email_address.to_string(),
                description: description.to_string(),
                active: true,
                visibility,
                app_password_id: Some(app_password.id),
                default_permission,
            });
            info!(
                "Inserted new category {} ({}) with app_password_id {}",
                inserted.id, email_address, app_password.id
            );
            inserted.id
        }
    });

    Ok(category_id)
}
