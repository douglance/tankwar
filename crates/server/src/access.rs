//! Cloudflare Access service-token assertions and server-owned agent grants.
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct AccessClaims {
    #[serde(rename = "type")]
    kind: String,
    sub: String,
    common_name: String,
    iat: u64,
    exp: u64,
}

#[derive(Deserialize)]
struct Grant { agent_id: String, scopes: Vec<String> }

pub fn verify_service(token: &str, issuer: &str, audience: &str, jwks: &str, grants: &str, scope: &str) -> Result<String, &'static str> {
    #[cfg(target_arch = "wasm32")]
    let now = worker::Date::now().as_millis() / 1000;
    #[cfg(not(target_arch = "wasm32"))]
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| "invalid_clock")?.as_secs();
    verify_service_at(token, issuer, audience, jwks, grants, scope, now)
}

fn verify_service_at(token: &str, issuer: &str, audience: &str, jwks: &str, grants: &str, scope: &str, now: u64) -> Result<String, &'static str> {
    if issuer.is_empty() || audience.is_empty() || token.len() > 16_384 { return Err("invalid_auth_configuration"); }
    let bindings: BTreeMap<String, Grant> = serde_json::from_str(grants).map_err(|_| "invalid_auth_configuration")?;
    let header = decode_header(token).map_err(|_| "invalid_token")?;
    if header.alg != Algorithm::RS256 { return Err("invalid_token"); }
    let kid = header.kid.as_deref().filter(|s| !s.is_empty()).ok_or("invalid_token")?;
    let keys: JwkSet = serde_json::from_str(jwks).map_err(|_| "invalid_auth_configuration")?;
    let key = DecodingKey::from_jwk(keys.find(kid).ok_or("invalid_token")?).map_err(|_| "invalid_token")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub", "iat"]);
    validation.leeway = 0;
    let claims = decode::<AccessClaims>(token, &key, &validation).map_err(|_| "invalid_token")?.claims;
    if claims.kind != "app" || !claims.sub.is_empty() || claims.common_name.is_empty() || claims.common_name.len() > 256 { return Err("invalid_token"); }
    if claims.iat > now || claims.exp <= now || claims.exp <= claims.iat || claims.exp.saturating_sub(claims.iat) > 3600 { return Err("invalid_token_lifetime"); }
    let grant = bindings.get(&claims.common_name).ok_or("unregistered_credential")?;
    if grant.agent_id.is_empty() || grant.agent_id.len() > 128 || !grant.agent_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("invalid_auth_configuration"); }
    if !grant.scopes.iter().any(|s| s == scope) { return Err("insufficient_scope"); }
    Ok(grant.agent_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_service_assertion_requires_signature_app_audience_and_grant() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/access-test-public.json")).unwrap();
        let jwks = fixture["jwks"].to_string();
        let grants = r#"{"client-a.access":{"agent_id":"agent-1","scopes":["queue:write","match:command"]}}"#;
        let token = |key: &str| fixture[key].as_str().unwrap();
        let check = |key| verify_service_at(token(key), "https://team.cloudflareaccess.com", "tankwar-app", &jwks, grants, "match:command", 4_102_444_500);
        assert_eq!(check("good"), Ok("agent-1".into()));
        for bad in ["bad_aud", "bad_type", "human_sub", "unknown_client", "future_iat", "long_lived"] { assert!(check(bad).is_err(), "{bad}"); }
        assert!(verify_service_at(token("good"), "https://wrong.cloudflareaccess.com", "tankwar-app", &jwks, grants, "match:command", 4_102_444_500).is_err());
        assert!(verify_service_at(token("good"), "https://team.cloudflareaccess.com", "tankwar-app", &jwks, grants, "match:observe", 4_102_444_500).is_err());
        assert!(verify_service_at(token("good"), "https://team.cloudflareaccess.com", "tankwar-app", &jwks, grants, "match:command", 4_102_444_801).is_err());
        let mut forged = token("good").to_owned();
        let last = forged.len()-2; forged.replace_range(last..last+1, if &forged[last..last+1] == "A" { "B" } else { "A" });
        assert!(verify_service_at(&forged, "https://team.cloudflareaccess.com", "tankwar-app", &jwks, grants, "match:command", 4_102_444_500).is_err());
    }
}
