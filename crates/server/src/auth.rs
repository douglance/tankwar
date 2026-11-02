//! Verification of externally issued, short-lived agent credentials.
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    agent_id: String,
    scope: String,
}

/// Verify against a configured issuer and public key set. Configuration is never
/// obtained from the untrusted token. Revocation still requires an external check.
pub fn verify_agent(token: &str, issuer: &str, audience: &str, jwks: &str, required_scope: &str) -> Result<String, &'static str> {
    if issuer.is_empty() || audience.is_empty() || token.len() > 16_384 { return Err("invalid_auth_configuration"); }
    let header = decode_header(token).map_err(|_| "invalid_token")?;
    if header.alg != Algorithm::RS256 { return Err("invalid_token"); }
    let key_id = header.kid.as_deref().filter(|v| !v.is_empty()).ok_or("invalid_token")?;
    let keys: JwkSet = serde_json::from_str(jwks).map_err(|_| "invalid_auth_configuration")?;
    let jwk = keys.find(key_id).ok_or("invalid_token")?;
    let key = DecodingKey::from_jwk(jwk).map_err(|_| "invalid_token")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.leeway = 0;
    let claims = decode::<Claims>(token, &key, &validation).map_err(|_| "invalid_token")?.claims;
    if claims.sub.is_empty() || claims.agent_id.is_empty() || claims.agent_id.len() > 128 || !claims.scope.split_ascii_whitespace().any(|s| s == required_scope) {
        return Err("insufficient_scope");
    }
    Ok(claims.agent_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    const JWKS: &str = r#"{"keys":[{"kty":"RSA","kid":"test","n":"7nTjxvOdTrCEKymOSY8Q8RrQRCuUQa4a8hBK1peS_tQK0kFvexiiGg9B2LOR9q5qZPgcxgKxz2QL0p5Kjdo9ZkrUT6Mpi_r46yg1XlVnUJQRpj0Aw0oZjHT5EwIGjxrWl2peGGWr1P1_Yh8e6vEk9jxd3_RX3jcocG9gwrxtstqxpQRML3gsFtfVzyG3snILevUfGdhHm7qDAZWhjA9DvkKimyhg54nIwov8QRpZd55YyFNPyx9D7FfFpucDN06CYPwLSN-rgaLa8Q8hA8sdcQh9MnalJ4rNeLRs8SZi-z1nCW6ptzllJu-v92Mp0j8sgj79U9eBzp5M0TdGivZj2w","e":"AQAB"}]}"#;
    const GOOD: &str = r#"eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3QifQ.eyJpc3MiOiJodHRwczovL2lzc3Vlci5leGFtcGxlIiwic3ViIjoib3duZXIiLCJhZ2VudF9pZCI6ImFnZW50LTEiLCJhdWQiOiJ0YW5rd2FyIiwic2NvcGUiOiJtYXRjaDpjb21tYW5kIiwiZXhwIjo0MTAyNDQ0ODAwfQ.pENJGPhCcGRrUVQO06usFGPpBTxzaibyAynK2s6857Bh8BZ73YjxnFZoVFwpfFgsdSe_9TNL5gA4z4HG-dG_1E1Ex2TrO_kGWHox-AfhAgjLj90sQnfJo8FBMSHuaMkJ1QbnXUeluDf_W4cCfBZVI2_SZRQL-2-KgLume_-Rjd6A5AUmjsPn3oO2MYiAggCz78DxjgAKunY_XiAaluk--bai9ug8hJIUBWm5cSMVS33DxE2VvixkC01As3vYI61VrlBC-g5c-GLIrkIiRpogZxZK2jgdjWXKYYt9SAfDGd7Rr4IJhsYUHnQbHX6MirP0IkRRqneUSapiM3XO1fQSww"#;
    const BAD_AUDIENCE: &str = r#"eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3QifQ.eyJpc3MiOiJodHRwczovL2lzc3Vlci5leGFtcGxlIiwic3ViIjoib3duZXIiLCJhZ2VudF9pZCI6ImFnZW50LTEiLCJhdWQiOiJvdGhlciIsInNjb3BlIjoibWF0Y2g6Y29tbWFuZCIsImV4cCI6NDEwMjQ0NDgwMH0.s4d2xcV9SRcD66Kjyp9cVx_YGzZCSpLrHT5BSGr1XHqPPKJIGJp2J1op4gBsgSBQa6AYVaXVKdeu4EUntkmU6qO_MZtSXuFAHiIpQrubcVu0Onf1ZET7Bo7VqQPsgCeDN90fgv-uQKUWL4T0udcAZ-wceZXWPx6B-hmcNeEoOmNI09IhaNfBwml_oB_8EO2YonZxf4Hw8YnnL0uDmRjUNevK9RUxpZoRncbaq_yqZDj087fq5WuYyjbSapLfxS49ubHlmG3hbZt2RBA-vW9vP3hNc5jYhvfJGnPxM8a72J9ics298QVKAjY9LhkBii-nmxTI4pckTsyx3oNYIgvFxA"#;
    const EXPIRED: &str = r#"eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3QifQ.eyJpc3MiOiJodHRwczovL2lzc3Vlci5leGFtcGxlIiwic3ViIjoib3duZXIiLCJhZ2VudF9pZCI6ImFnZW50LTEiLCJhdWQiOiJ0YW5rd2FyIiwic2NvcGUiOiJtYXRjaDpjb21tYW5kIiwiZXhwIjoxfQ.J26vXxrq-bW_QqDUgA9uWI-0N0Neij0LFt73SUjMdG_2pdqah63EN3z_8zie2nH1aFG-nZF1L9nb_IHiHeXZOIiZQcidjWHUSI_F44hUfXTbE1132qAvOWy4OFDG-Ro0-yHGUZThJ4VCPn2gdN_oZi9gsFvTNdbDAbmV7d-wTHJgL1l3VxoABXV7Syi1jefChXja2m704ZZfEffIRhByWDE_cjFlLN5OH8m5Q1mm9hFkRThIUDZNMbw7Rd8Vh3m5Io21Rmj6ySHBLCd4YWPRlUkKqSm_BhOyp0YtDg0r8xeXByaNya1Roz0yNlssmieQ3Nvz4I1kRPg-EnF3coTWGg"#;
    const WRONG_SCOPE: &str = r#"eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3QifQ.eyJpc3MiOiJodHRwczovL2lzc3Vlci5leGFtcGxlIiwic3ViIjoib3duZXIiLCJhZ2VudF9pZCI6ImFnZW50LTEiLCJhdWQiOiJ0YW5rd2FyIiwic2NvcGUiOiJwcm9maWxlOnJlYWQiLCJleHAiOjQxMDI0NDQ4MDB9.Qj9zr_8-JOARaD7KdZ_jnED-0J2iKSQyGqb7qdgMpuHr-XULEykqbtr9_JmaEu7uVAlol0N-7VDbafwbVUFnNVMtRPX4KU-rGXcBhRXH3LLao9Ljd6lIhUtHoqZQ_9b-9H8tosLYxzm1b9HFnNfzK5JYooNMe202vNjPhPZydAPt_INSKzTm4bBYc09SBqQEgvlai6maVTfwCOHh_WiLDYqS1Ey9No9h0zQKFR0yxDyoXEj7uRUGTcIiOVKS99rEYZsWY_9ra23y6hvwWJ9L3pPUuyDaUIcSsTYgB8KluTEp4dd4T7xeyupNBfS5WbInsmPcASLfv0NAzdVWTMok3Q"#;
    #[test]
    fn authenticates_only_signed_scoped_unexpired_tokens() {
        let check = |token| verify_agent(token, "https://issuer.example", "tankwar", JWKS, "match:command");
        assert_eq!(check(GOOD), Ok("agent-1".to_string()));
        assert!(check(BAD_AUDIENCE).is_err());
        assert!(check(EXPIRED).is_err());
        assert!(check(WRONG_SCOPE).is_err());
        assert!(verify_agent(GOOD, "https://different.example", "tankwar", JWKS, "match:command").is_err());
        let mut forged = GOOD.to_string(); forged.replace_range(20..21, "X");
        assert!(check(&forged).is_err());
    }
}
