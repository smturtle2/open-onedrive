use anyhow::{Context, Result};
use serde::Deserialize;
use url::Url;

#[derive(Debug, Clone)]
pub struct GraphClient {
    http: reqwest::Client,
}

impl Default for GraphClient {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }
}

impl GraphClient {
    pub fn browser_url() -> Url {
        Url::parse("https://onedrive.live.com/").expect("static browser url is valid")
    }

    pub async fn drive_delta(&self, access_token: &str, next_link: Option<&str>) -> Result<DeltaPage> {
        let url = next_link
            .unwrap_or("https://graph.microsoft.com/v1.0/me/drive/root/delta")
            .to_string();
        let payload: DeltaPayload = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .context("drive delta request failed")?
            .error_for_status()
            .context("drive delta request was rejected")?
            .json()
            .await
            .context("unable to deserialize drive delta response")?;

        Ok(DeltaPage {
            items: payload.value,
            next_link: payload.next_link,
            delta_link: payload.delta_link,
        })
    }

    pub async fn create_upload_session(
        &self,
        access_token: &str,
        parent_item_id: &str,
        file_name: &str,
    ) -> Result<UploadSession> {
        let endpoint = format!(
            "https://graph.microsoft.com/v1.0/me/drive/items/{parent_item_id}:/{file_name}:/createUploadSession"
        );
        let payload: UploadSession = self
            .http
            .post(endpoint)
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "item": { "@microsoft.graph.conflictBehavior": "rename" } }))
            .send()
            .await
            .context("upload session request failed")?
            .error_for_status()
            .context("upload session request was rejected")?
            .json()
            .await
            .context("unable to deserialize upload session response")?;
        Ok(payload)
    }
}

#[derive(Debug, Clone)]
pub struct DeltaPage {
    pub items: Vec<DriveItem>,
    pub next_link: Option<String>,
    pub delta_link: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveItem {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSession {
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
    #[serde(rename = "expirationDateTime")]
    pub expiration_date_time: String,
}

#[derive(Debug, Deserialize)]
struct DeltaPayload {
    #[serde(default)]
    value: Vec<DriveItem>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    delta_link: Option<String>,
}

