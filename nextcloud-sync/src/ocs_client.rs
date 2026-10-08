use crate::config::Config;
use crate::models::{OcsResponse, OcsUserDetails};
use std::collections::HashMap;

#[derive(Clone)]
pub struct OcsClient {
    client: reqwest::Client,
    nc_url: String,
    nc_user: String,
    nc_app_password: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OcsError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("OCS API error {code}: {message}")]
    OcsApi { code: i64, message: String },
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl OcsClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(40))
                .build()
                .expect("Failed to initialize reqwest client"),
            nc_url: config.nc_url.clone(),
            nc_user: config.nc_user.clone(),
            nc_app_password: config.nc_app_password.clone(),
        }
    }

    /// Fetch single user details: GET /ocs/v2.php/cloud/users/{uid}
    pub async fn get_user(&self, uid: &str) -> Result<Option<OcsUserDetails>, OcsError> {
        let url = format!("{}/ocs/v2.php/cloud/users/{}", self.nc_url, urlencoding_encode(uid));
        let res = self
            .client
            .get(&url)
            .basic_auth(&self.nc_user, Some(&self.nc_app_password))
            .header("OCS-APIRequest", "true")
            .header("Accept", "application/json")
            .send()
            .await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let resp_body: OcsResponse<OcsUserDetails> = res.json().await?;
        if resp_body.ocs.meta.statuscode != 100 && resp_body.ocs.meta.statuscode != 200 {
            if resp_body.ocs.meta.statuscode == 998 {
                // Not found code in some Nextcloud OCS versions
                return Ok(None);
            }
            return Err(OcsError::OcsApi {
                code: resp_body.ocs.meta.statuscode,
                message: resp_body.ocs.meta.message.unwrap_or_default(),
            });
        }

        Ok(Some(resp_body.ocs.data))
    }

    /// Fetch all users with details: GET /ocs/v2.php/cloud/users/details
    pub async fn list_all_users_details(&self) -> Result<Vec<OcsUserDetails>, OcsError> {
        // Nextcloud /ocs/v2.php/cloud/users/details returns { users: { "uid1": {...}, "uid2": {...} } }
        #[derive(serde::Deserialize)]
        struct UsersMapWrapper {
            users: HashMap<String, OcsUserDetails>,
        }

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
                .send()
                .await?;

            let resp_body: OcsResponse<UsersMapWrapper> = res.json().await?;
            if resp_body.ocs.meta.statuscode != 100 && resp_body.ocs.meta.statuscode != 200 {
                return Err(OcsError::OcsApi {
                    code: resp_body.ocs.meta.statuscode,
                    message: resp_body.ocs.meta.message.unwrap_or_default(),
                });
            }

            let batch_count = resp_body.ocs.data.users.len();
            for (_, user) in resp_body.ocs.data.users {
                all_users.push(user);
            }

            if batch_count < limit {
                break;
            }
            offset += limit;
        }

        Ok(all_users)
    }
}

fn urlencoding_encode(s: &str) -> String {
    // Simple URL-encoding for path component
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
