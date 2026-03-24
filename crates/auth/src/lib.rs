use anyhow::{Context, Result};
use oauth2::{CsrfToken, PkceCodeChallenge};
use url::Url;

const AUTH_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Clone)]
pub struct LoginRequest {
    pub authorize_url: String,
    pub csrf_state: String,
    pub pkce_verifier: String,
    pub redirect_uri: String,
    pub token_url: String,
}

pub fn build_authorization_request(client_id: &str, loopback_port: u16) -> Result<LoginRequest> {
    let redirect_uri = format!("http://127.0.0.1:{loopback_port}/callback");
    let csrf = CsrfToken::new_random();
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

    let mut url = Url::parse(AUTH_BASE).context("unable to parse microsoft auth endpoint")?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("response_mode", "query")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", "offline_access Files.ReadWrite.All User.Read")
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge.as_str())
        .append_pair("state", csrf.secret());

    Ok(LoginRequest {
        authorize_url: url.into(),
        csrf_state: csrf.secret().to_owned(),
        pkce_verifier: verifier.secret().to_owned(),
        redirect_uri,
        token_url: TOKEN_BASE.to_string(),
    })
}

