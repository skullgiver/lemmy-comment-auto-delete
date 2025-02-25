use std::fmt::{Display, Formatter};
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// An object representing a single comment. Simplified.
#[derive(Deserialize)]
pub(crate) struct Comment {
    pub(crate) id: i64,
    pub(crate) content: String,
    pub(crate) removed: bool,
    pub(crate) deleted: Option<bool>,
    #[serde(deserialize_with = "crate::helper::deserialize_date")]
    pub(crate) published: DateTime<Utc>,
}

impl Comment {
    /// A user-readable item ID, used for debugging
    pub fn item_id(&self) -> String {
        format!("{}", self.id)
    }
    /// A subselection of a comment's contents, for use in debugging and printing.
    pub fn short_content(&self) -> &str {
        &self.content[..100.min(self.content.len())]
    }
}

impl Display for Comment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let is_deleted = match self.deleted {
            Some(true) => " [DELETED]",
            Some(false) => "",
            None => "[DELETED?]"
        };

        let is_removed = if self.removed { "[REMOVED]" } else { "" };

        write!(f, "Comment {}{}{}: [{}] {}", self.id, is_removed, is_deleted, self.published, self.short_content())
    }
}