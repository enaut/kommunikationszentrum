use log::error;
use spacetimedb::ProcedureContext;

use crate::models::config::stalwart_config;

pub fn resolve_stalwart_credentials(
    ctx: &mut ProcedureContext,
) -> Result<(String, String), String> {
    let config = ctx.with_tx(|tx| tx.db.stalwart_config().id().find(&0));
    if let Some(config) = config {
        let endpoint = if config.jmap_url.ends_with("/jmap") {
            config.jmap_url
        } else {
            format!("{}/jmap", config.jmap_url)
        };
        return Ok((endpoint, config.admin_token));
    }

    if let (Ok(url), Ok(token)) = (
        std::env::var("STALWART_JMAP_URL"),
        std::env::var("STALWART_ADMIN_TOKEN"),
    ) {
        let endpoint = if url.ends_with("/jmap") {
            url.trim_end_matches('/').to_string()
        } else {
            format!("{}/jmap", url.trim_end_matches('/'))
        };
        return Ok((endpoint, token));
    }

    Err("Stalwart JMAP is not configured. Set configuration via set_stalwart_config or STALWART_JMAP_URL environment variable.".to_string())
}

pub fn send_stalwart_jmap_request(
    ctx: &mut ProcedureContext,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (endpoint, admin_token) = resolve_stalwart_credentials(ctx)?;
    let body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize JMAP payload: {}", e);
        format!("Failed to serialize JMAP payload: {}", e)
    })?;

    let request = spacetimedb::http::Request::builder()
        .uri(endpoint)
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_token))
        .extension(spacetimedb::http::Timeout(
            spacetimedb::TimeDuration::from_micros(30_000_000),
        ))
        .body(body)
        .map_err(|e| format!("Failed to build HTTP request: {:?}", e))?;

    let response = ctx.http.send(request).map_err(|e| {
        error!("Failed to perform request: {}", e);
        format!("HTTP send failed: {:?}", e)
    })?;

    let (parts, body) = response.into_parts();
    let status = parts.status.as_u16();
    if status < 200 || status >= 300 {
        let err_body = body.into_string_lossy();
        error!(
            "JMAP HTTP request failed with status {}: {}",
            status, err_body
        );
        return Err(format!(
            "JMAP request returned error HTTP status {}: {}",
            status, err_body
        ));
    }

    let body_bytes = body.into_bytes();
    let parsed_json: serde_json::Value = serde_json::from_slice(&body_bytes).map_err(|e| {
        error!("Failed to parse JMAP JSON response: {}", e);
        format!("Failed to parse JMAP JSON response: {}", e)
    })?;

    Ok(parsed_json)
}

pub fn jmap_method_result<'a>(
    res: &'a serde_json::Value,
    method_name: &str,
    client_id: &str,
) -> Result<&'a serde_json::Value, String> {
    let method_responses = res["methodResponses"]
        .as_array()
        .ok_or_else(|| "Response missing 'methodResponses' array".to_string())?;

    for call in method_responses {
        if let Some(arr) = call.as_array() {
            if arr.len() >= 3 && arr[0] == method_name && arr[2] == client_id {
                return Ok(&arr[1]);
            }
        }
    }

    Err(format!(
        "No method response matching name '{}' and client id '{}'",
        method_name, client_id
    ))
}
