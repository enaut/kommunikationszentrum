use crate::services::stalwart::client::send_stalwart_jmap_request;
use spacetimedb::ProcedureContext;

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
    ctx: &mut ProcedureContext,
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
