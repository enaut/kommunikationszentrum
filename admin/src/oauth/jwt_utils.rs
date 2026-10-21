use base64::Engine;
use serde::{Deserialize, Serialize};

/// Result of decoding a (non-encrypted) JWT: header & claims as generic JSON plus raw signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecodedJwt {
    pub header: serde_json::Value,
    pub claims: serde_json::Value,
    pub signature_b64: String,
    pub header_raw: String,
    pub claims_raw: String,
}

/// Decode Base64URL string (with or without padding) to bytes.
pub fn b64url_decode(input: &str) -> Result<Vec<u8>, String> {
    let mut s = input.replace('-', "+").replace('_', "/");
    while s.len() % 4 != 0 {
        s.push('=');
    }
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("Base64 decode error: {e}"))
}

/// Decode a (non-encrypted) JWT string without verifying the signature.
///
/// NOTE: This is a base64url decode + JSON parse only. Do not rely on this for security
/// decisions; signature and claim validation must already have been done during login.
pub fn decode_jwt(token: &str) -> Result<DecodedJwt, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Malformed JWT (expected 3 parts)".into());
    }
    let (h_b64, c_b64, s_b64) = (parts[0], parts[1], parts[2]);

    let header_raw =
        b64url_decode(h_b64).and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))?;
    let claims_raw =
        b64url_decode(c_b64).and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))?;

    let header_json: serde_json::Value =
        serde_json::from_str(&header_raw).map_err(|e| format!("Header JSON parse error: {e}"))?;
    let claims_json: serde_json::Value =
        serde_json::from_str(&claims_raw).map_err(|e| format!("Claims JSON parse error: {e}"))?;

    Ok(DecodedJwt {
        header: header_json,
        claims: claims_json,
        signature_b64: s_b64.to_string(),
        header_raw,
        claims_raw,
    })
}

/// Returns current unix timestamp in seconds.
pub fn current_unix_timestamp() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::new_0().get_time() / 1000.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Extract numeric `exp` claim from JWT claims value.
pub fn get_jwt_exp(claims: &serde_json::Value) -> Option<u64> {
    let exp_val = claims.get("exp")?;
    exp_val
        .as_u64()
        .or_else(|| {
            exp_val
                .as_i64()
                .and_then(|i| if i >= 0 { Some(i as u64) } else { None })
        })
        .or_else(|| {
            exp_val
                .as_f64()
                .and_then(|f| if f >= 0.0 { Some(f as u64) } else { None })
        })
}

/// Returns whether a decoded JWT is unexpired with leeway in seconds.
/// If `exp` claim is absent, returns `true`.
pub fn is_jwt_unexpired_at(claims: &serde_json::Value, now: u64, leeway_secs: u64) -> bool {
    if let Some(exp) = get_jwt_exp(claims) {
        exp > now.saturating_add(leeway_secs)
    } else {
        true
    }
}

/// Returns whether a decoded JWT is unexpired using the current time and a leeway.
#[allow(dead_code)]
pub fn is_jwt_unexpired(claims: &serde_json::Value, leeway_secs: u64) -> bool {
    is_jwt_unexpired_at(claims, current_unix_timestamp(), leeway_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_jwt_success() {
        // {"alg":"HS256","typ":"JWT"} -> eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
        // {"sub":"1234567890","name":"John Doe","iat":1516239022} -> eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let decoded = decode_jwt(token).expect("decoding should succeed");
        assert_eq!(decoded.header["alg"], "HS256");
        assert_eq!(decoded.claims["sub"], "1234567890");
        assert_eq!(decoded.claims["name"], "John Doe");
        assert_eq!(
            decoded.signature_b64,
            "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        );
    }

    #[test]
    fn test_decode_jwt_malformed() {
        assert!(decode_jwt("not.enough.parts.really").is_err());
        assert!(decode_jwt("only_one_part").is_err());
        assert!(decode_jwt("two.parts").is_err());
    }

    #[test]
    fn test_jwt_exp_checking() {
        let claims = serde_json::json!({
            "exp": 1700000000u64,
        });
        assert_eq!(get_jwt_exp(&claims), Some(1700000000));
        assert!(is_jwt_unexpired_at(&claims, 1699999900, 30));
        // Exactly at exp - 30 -> boundary test
        assert!(!is_jwt_unexpired_at(&claims, 1699999970, 30));
        // Already expired
        assert!(!is_jwt_unexpired_at(&claims, 1700000001, 30));

        // Floating point exp
        let claims_float = serde_json::json!({
            "exp": 1700000000.0,
        });
        assert_eq!(get_jwt_exp(&claims_float), Some(1700000000));
        assert!(is_jwt_unexpired_at(&claims_float, 1699999900, 30));

        // Missing exp -> considered unexpired
        let claims_no_exp = serde_json::json!({
            "sub": "user123",
        });
        assert_eq!(get_jwt_exp(&claims_no_exp), None);
        assert!(is_jwt_unexpired_at(&claims_no_exp, 2000000000, 30));
    }
}
