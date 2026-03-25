use anyhow::{Context, Result};
use reqwest::{Client, redirect::Policy};
use serde::Deserialize;
use url::Url;

const DELTA_ENDPOINT: &str = "https://graph.microsoft.com/v1.0/me/drive/root/delta";
const ME_ENDPOINT: &str = "https://graph.microsoft.com/v1.0/me?$select=displayName,userPrincipalName";

#[derive(Debug, Clone)]
pub struct GraphClient {
    http: Client,
}

impl Default for GraphClient {
    fn default() -> Self {
        Self {
            http: Client::builder()
                .redirect(Policy::none())
                .build()
                .expect("graph client should build"),
        }
    }
}

impl GraphClient {
    pub fn browser_url() -> Url {
        Url::parse("https://onedrive.live.com/").expect("static browser url is valid")
    }

    pub async fn drive_delta(
        &self,
        access_token: &str,
        next_link: Option<&str>,
    ) -> Result<DeltaPage> {
        let url = next_link.unwrap_or(DELTA_ENDPOINT).to_string();
        let payload: DeltaPayload = self
            .get_json(&url, access_token)
            .await
            .context("unable to load drive delta page")?;

        Ok(DeltaPage {
            items: payload.value,
            next_link: payload.next_link,
            delta_link: payload.delta_link,
        })
    }

    pub async fn collect_drive_delta(
        &self,
        access_token: &str,
        cursor: Option<&str>,
    ) -> Result<DeltaCollection> {
        let mut next_link = cursor.map(ToOwned::to_owned);
        let mut pages = 0usize;
        let mut items = Vec::new();
        let mut final_delta_link = None;

        loop {
            let page = self
                .drive_delta(access_token, next_link.as_deref())
                .await
                .with_context(|| format!("unable to collect delta page {}", pages + 1))?;
            pages += 1;
            items.extend(page.items);
            final_delta_link = page.delta_link.or(final_delta_link);
            match page.next_link {
                Some(link) => next_link = Some(link),
                None => break,
            }
        }

        Ok(DeltaCollection {
            items,
            delta_link: final_delta_link,
            pages,
        })
    }

    pub async fn current_user(&self, access_token: &str) -> Result<UserProfile> {
        self.get_json(ME_ENDPOINT, access_token)
            .await
            .context("unable to fetch Microsoft profile")
    }

    pub async fn download_content(&self, access_token: &str, item_id: &str) -> Result<Vec<u8>> {
        let endpoint = format!("https://graph.microsoft.com/v1.0/me/drive/items/{item_id}/content");
        self.http
            .get(endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .context("content download request failed")?
            .error_for_status()
            .context("content download request was rejected")?
            .bytes()
            .await
            .context("unable to read downloaded content")?
            .to_vec()
            .pipe(Ok)
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
        self.http
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
            .context("unable to deserialize upload session response")
    }

    async fn get_json<T>(&self, url: &str, access_token: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .with_context(|| format!("request failed for {url}"))?
            .error_for_status()
            .with_context(|| format!("request rejected for {url}"))?
            .json()
            .await
            .with_context(|| format!("unable to decode response from {url}"))
    }
}

#[derive(Debug, Clone)]
pub struct DeltaPage {
    pub items: Vec<DriveItem>,
    pub next_link: Option<String>,
    pub delta_link: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeltaCollection {
    pub items: Vec<DriveItem>,
    pub delta_link: Option<String>,
    pub pages: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub file: Option<FileFacet>,
    #[serde(default)]
    pub folder: Option<FolderFacet>,
    #[serde(default)]
    pub deleted: Option<DeletedFacet>,
    #[serde(default, rename = "parentReference")]
    pub parent_reference: Option<ParentReference>,
    #[serde(default, rename = "lastModifiedDateTime")]
    pub last_modified_date_time: Option<String>,
    #[serde(default, rename = "webUrl")]
    pub web_url: Option<String>,
    #[serde(default, rename = "@microsoft.graph.downloadUrl")]
    pub download_url: Option<String>,
}

impl DriveItem {
    pub fn is_deleted(&self) -> bool {
        self.deleted.is_some()
    }

    pub fn is_directory(&self) -> bool {
        self.folder.is_some()
    }

    pub fn parent_remote_id(&self) -> Option<&str> {
        self.parent_reference.as_ref()?.id.as_deref()
    }

    pub fn normalized_path(&self) -> Option<String> {
        if self.is_deleted() {
            return None;
        }

        let name = self.name.as_deref()?;
        if name.eq_ignore_ascii_case("root") {
            return Some("/".to_string());
        }

        let parent = self
            .parent_reference
            .as_ref()
            .and_then(|reference| reference.path.as_deref())
            .unwrap_or("");
        let relative_parent = parent
            .strip_prefix("/drive/root:")
            .or_else(|| parent.strip_prefix("drive/root:"))
            .unwrap_or(parent);

        Some(join_path(relative_parent, name))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileFacet {
    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub hashes: Option<FileHashes>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileHashes {
    #[serde(default, rename = "quickXorHash")]
    pub quick_xor_hash: Option<String>,
    #[serde(default, rename = "sha1Hash")]
    pub sha1_hash: Option<String>,
    #[serde(default, rename = "sha256Hash")]
    pub sha256_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FolderFacet {
    #[serde(default, rename = "childCount")]
    pub child_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeletedFacet {}

#[derive(Debug, Clone, Deserialize)]
pub struct ParentReference {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, rename = "driveId")]
    pub drive_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSession {
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
    #[serde(rename = "expirationDateTime")]
    pub expiration_date_time: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserProfile {
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default, rename = "userPrincipalName")]
    pub user_principal_name: Option<String>,
}

impl UserProfile {
    pub fn account_label(&self) -> String {
        self.display_name
            .clone()
            .or_else(|| self.user_principal_name.clone())
            .unwrap_or_else(|| "Microsoft account".to_string())
    }
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

fn join_path(parent: &str, name: &str) -> String {
    let trimmed_parent = parent.trim_matches('/');
    let trimmed_name = name.trim_matches('/');
    if trimmed_parent.is_empty() {
        format!("/{trimmed_name}")
    } else if trimmed_name.is_empty() {
        format!("/{trimmed_parent}")
    } else {
        format!("/{trimmed_parent}/{trimmed_name}")
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::{DriveItem, FolderFacet, ParentReference};

    #[test]
    fn normalizes_drive_root_relative_path() {
        let item = DriveItem {
            id: Some("file-1".into()),
            name: Some("Notes.txt".into()),
            size: Some(32),
            file: None,
            folder: None,
            deleted: None,
            parent_reference: Some(ParentReference {
                id: Some("parent-1".into()),
                path: Some("/drive/root:/Documents/Work".into()),
                drive_id: None,
            }),
            last_modified_date_time: None,
            web_url: None,
            download_url: None,
        };

        assert_eq!(item.normalized_path().as_deref(), Some("/Documents/Work/Notes.txt"));
    }

    #[test]
    fn root_folder_maps_to_root_path() {
        let item = DriveItem {
            id: Some("root".into()),
            name: Some("root".into()),
            size: Some(0),
            file: None,
            folder: Some(FolderFacet { child_count: Some(5) }),
            deleted: None,
            parent_reference: None,
            last_modified_date_time: None,
            web_url: None,
            download_url: None,
        };

        assert_eq!(item.normalized_path().as_deref(), Some("/"));
        assert!(item.is_directory());
    }
}
