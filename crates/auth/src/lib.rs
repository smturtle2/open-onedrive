use anyhow::{Context, Result, bail};
use oauth2::basic::BasicClient;
use oauth2::reqwest;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

const AUTH_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const REQUESTED_SCOPES: &[&str] = &["offline_access", "Files.ReadWrite.All", "User.Read"];

#[derive(Debug, Clone)]
pub struct LoginRequest {
    pub authorize_url: String,
    pub csrf_state: String,
    pub pkce_verifier: String,
    pub redirect_uri: String,
    pub token_url: String,
    pub requested_scope: String,
}

#[derive(Debug, Clone)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_seconds: Option<u64>,
    pub scope: String,
    pub token_type: String,
    pub id_token: Option<String>,
}

pub fn build_authorization_request(client_id: &str, loopback_port: u16) -> Result<LoginRequest> {
    validate_client_id(client_id)?;
    let redirect_uri = redirect_uri(loopback_port);
    let client = oauth_client(client_id, &redirect_uri)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

    let mut request = client.authorize_url(CsrfToken::new_random);
    for scope in REQUESTED_SCOPES {
        request = request.add_scope(Scope::new((*scope).to_string()));
    }
    let (authorize_url, csrf) = request.set_pkce_challenge(challenge).url();

    Ok(LoginRequest {
        authorize_url: authorize_url.to_string(),
        csrf_state: csrf.secret().to_owned(),
        pkce_verifier: verifier.secret().to_owned(),
        redirect_uri,
        token_url: TOKEN_BASE.to_string(),
        requested_scope: REQUESTED_SCOPES.join(" "),
    })
}

pub async fn exchange_authorization_code(
    client_id: &str,
    code: &str,
    pkce_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenSet> {
    validate_client_id(client_id)?;
    if code.trim().is_empty() {
        bail!("authorization code cannot be empty");
    }
    if pkce_verifier.trim().is_empty() {
        bail!("PKCE verifier cannot be empty");
    }

    let client = oauth_client(client_id, redirect_uri)?;
    let http_client = oauth_http_client()?;
    let token = client
        .exchange_code(AuthorizationCode::new(code.trim().to_string()))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.trim().to_string()))
        .request_async(&http_client)
        .await
        .context("authorization code exchange failed")?;

    Ok(to_token_set(&token))
}

pub async fn refresh_access_token(
    client_id: &str,
    refresh_token: &str,
    redirect_uri: &str,
) -> Result<TokenSet> {
    validate_client_id(client_id)?;
    if refresh_token.trim().is_empty() {
        bail!("refresh token cannot be empty");
    }

    let client = oauth_client(client_id, redirect_uri)?;
    let http_client = oauth_http_client()?;
    let token = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.trim().to_string()))
        .request_async(&http_client)
        .await
        .context("refresh token exchange failed")?;

    Ok(to_token_set(&token))
}

pub fn redirect_uri(loopback_port: u16) -> String {
    format!("http://127.0.0.1:{loopback_port}/callback")
}

fn oauth_client(client_id: &str, redirect_uri: &str) -> Result<
    BasicClient<
        oauth2::EndpointSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointSet,
    >,
> {
    Ok(BasicClient::new(ClientId::new(client_id.trim().to_string()))
        .set_auth_uri(AuthUrl::new(AUTH_BASE.to_string()).context("invalid auth endpoint")?)
        .set_token_uri(TokenUrl::new(TOKEN_BASE.to_string()).context("invalid token endpoint")?)
        .set_redirect_uri(
            RedirectUrl::new(redirect_uri.to_string()).context("invalid loopback redirect URI")?,
        ))
}

fn oauth_http_client() -> Result<reqwest::Client> {
    reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("unable to build OAuth HTTP client")
}

fn validate_client_id(client_id: &str) -> Result<()> {
    if client_id.trim().is_empty() {
        bail!("client ID cannot be empty");
    }
    Ok(())
}

fn to_token_set<T>(token: &T) -> TokenSet
where
    T: TokenResponse,
{
    TokenSet {
        access_token: token.access_token().secret().to_string(),
        refresh_token: token.refresh_token().map(|value| value.secret().to_string()),
        expires_in_seconds: token.expires_in().map(|value| value.as_secs()),
        scope: token
            .scopes()
            .map(|values| {
                values
                    .iter()
                    .map(|scope| scope.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| REQUESTED_SCOPES.join(" ")),
        token_type: format!("{:?}", token.token_type()),
        id_token: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{REQUESTED_SCOPES, build_authorization_request, redirect_uri};

    #[test]
    fn builds_loopback_redirect_uri() {
        assert_eq!(redirect_uri(53682), "http://127.0.0.1:53682/callback");
    }

    #[test]
    fn builds_authorization_request() {
        let request = build_authorization_request("client-id", 53682).expect("auth request");
        assert!(request.authorize_url.starts_with(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?"
        ));
        assert_eq!(request.redirect_uri, "http://127.0.0.1:53682/callback");
        assert_eq!(request.requested_scope, REQUESTED_SCOPES.join(" "));
        assert!(!request.csrf_state.is_empty());
        assert!(!request.pkce_verifier.is_empty());
    }
}
