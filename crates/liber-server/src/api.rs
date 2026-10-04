use liber_core::model::{Attachment, Bookmark};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ApiAttachment {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiBookmark {
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_id: Option<i64>,
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub folder: String,
    pub created_at: String,
    pub updated_at: String,
    pub has_markdown: bool,
    pub has_archive: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ApiAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_opened_at: Option<String>,
}

impl From<&Bookmark> for ApiBookmark {
    fn from(b: &Bookmark) -> Self {
        Self {
            uuid: b.uuid.to_string(),
            short_id: b.short_id,
            url: b.url.clone(),
            title: b.title.clone(),
            description: b.description.clone(),
            tags: b.tags.clone(),
            folder: b.folder.clone(),
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
            has_markdown: b.markdown_file.is_some(),
            has_archive: b.archive_file.is_some(),
            attachments: b
                .attachments
                .iter()
                .map(|a: &Attachment| ApiAttachment {
                    name: a.name.clone(),
                })
                .collect(),
            open_count: (b.open_count > 0).then_some(b.open_count),
            last_opened_at: b.last_opened_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ListParams {
    pub q: Option<String>,
    pub sort: Option<String>,
    pub tag: Option<String>,
    pub folder: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub deep: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub bookmarks: Vec<ApiBookmark>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AddRequest {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub folder: Option<String>,
    #[serde(default)]
    pub markdown: bool,
    #[serde(default)]
    pub archive: bool,
    #[serde(default)]
    pub confirm_dup: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub folder: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DeleteParams {
    pub confirm: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}
