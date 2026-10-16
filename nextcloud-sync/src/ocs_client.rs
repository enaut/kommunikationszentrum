use crate::config::Config;
use crate::models::{OcsResponse, OcsUserDetails, UsersWrapper};

#[derive(Clone)]
pub struct OcsClient {
    client: reqwest::Client,
    nc_url: String,
    nc_user: String,
    nc_app_password: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OcsError {
    #[error("HTTP network/transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP error status {status}: {body}")]
    HttpStatus {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("OCS API error {code}: {message}")]
    OcsApi { code: i64, message: String },
    #[error("JSON deserialization error: {source} (raw body: {body})")]
    JsonWithBody {
        source: serde_json::Error,
        body: String,
    },
}

impl OcsClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(40))
                .build()
                .expect("Failed to initialize reqwest client"),
            nc_url: config.nc_url.trim_end_matches('/').to_string(),
            nc_user: config.nc_user.clone(),
            nc_app_password: config.nc_app_password.clone(),
        }
    }

    /// Fetch single user details: GET /ocs/v2.php/cloud/users/{uid}
    pub async fn get_user(&self, uid: &str) -> Result<Option<OcsUserDetails>, OcsError> {
        let url = format!(
            "{}/ocs/v2.php/cloud/users/{}",
            self.nc_url,
            urlencoding_encode(uid)
        );
        let res = self
            .client
            .get(&url)
            .basic_auth(&self.nc_user, Some(&self.nc_app_password))
            .header("OCS-APIRequest", "true")
            .header("Accept", "application/json")
            .header("User-Agent", "Kommunikationszentrum-Nextcloud-Sync/1.0")
            .send()
            .await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let status = res.status();
        let body = res.text().await?;

        let resp_body: OcsResponse<OcsUserDetails> = match serde_json::from_str(&body) {
            Ok(parsed) => parsed,
            Err(e) => {
                if !status.is_success() {
                    return Err(OcsError::HttpStatus { status, body });
                }
                return Err(OcsError::JsonWithBody { source: e, body });
            }
        };

        if resp_body.ocs.meta.statuscode != 100 && resp_body.ocs.meta.statuscode != 200 {
            if resp_body.ocs.meta.statuscode == 998 || resp_body.ocs.meta.statuscode == 404 {
                // Not found code in Nextcloud OCS
                return Ok(None);
            }
            return Err(OcsError::OcsApi {
                code: resp_body.ocs.meta.statuscode,
                message: resp_body.ocs.meta.message.unwrap_or_default(),
            });
        }

        Ok(resp_body.ocs.data)
    }

    /// Fetch all users with details: GET /ocs/v2.php/cloud/users/details
    pub async fn list_all_users_details(&self) -> Result<Vec<OcsUserDetails>, OcsError> {
        let mut all_users = Vec::new();
        let mut offset = 0;
        let limit = 100;

        loop {
            let url = format!(
                "{}/ocs/v2.php/cloud/users/details?offset={}&limit={}",
                self.nc_url, offset, limit
            );

            let res = self
                .client
                .get(&url)
                .basic_auth(&self.nc_user, Some(&self.nc_app_password))
                .header("OCS-APIRequest", "true")
                .header("Accept", "application/json")
                .header("User-Agent", "Kommunikationszentrum-Nextcloud-Sync/1.0")
                .send()
                .await?;

            let status = res.status();
            let link_header = res
                .headers()
                .get("link")
                .and_then(|h| h.to_str().ok())
                .map(str::to_string);

            let body = res.text().await?;

            let resp_body: OcsResponse<UsersWrapper> = match serde_json::from_str(&body) {
                Ok(parsed) => parsed,
                Err(e) => {
                    if !status.is_success() {
                        return Err(OcsError::HttpStatus { status, body });
                    }
                    return Err(OcsError::JsonWithBody { source: e, body });
                }
            };

            if resp_body.ocs.meta.statuscode != 100 && resp_body.ocs.meta.statuscode != 200 {
                return Err(OcsError::OcsApi {
                    code: resp_body.ocs.meta.statuscode,
                    message: resp_body.ocs.meta.message.unwrap_or_default(),
                });
            }

            let users = resp_body.ocs.data.map(|d| d.users).unwrap_or_default();
            let batch_count = users.len();

            for user in users {
                all_users.push(user);
            }

            // Pagination termination checks:
            // 1. If 0 users returned in this batch, reached the end
            if batch_count == 0 {
                break;
            }

            // 2. Nextcloud 35+ Link header pagination check:
            // If Link header is present, continue only if it includes rel="next".
            if let Some(ref link) = link_header {
                if !link.contains("rel=\"next\"") && !link.contains("rel=next") {
                    break;
                }
            } else if batch_count < limit {
                // Fallback for endpoints without Link header:
                // fewer than limit items means we have fetched the last batch.
                break;
            }

            offset += limit;
        }

        Ok(all_users)
    }
}

fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
}
