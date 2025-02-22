use std::fmt::{Display, Formatter};
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// An object representing a single post. Simplified.
#[derive(Deserialize)]
pub(crate) struct Post {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) removed: bool,
    pub(crate) deleted: bool,
    #[serde(deserialize_with = "crate::helper::deserialize_date")]
    pub(crate) published: DateTime<Utc>,
}

impl Post {
    /// A user-readable item ID, used for debugging
    pub fn item_id(&self) -> String {
        format!("{}", self.id)
    }
}

impl Display for Post {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Post {}{}{}: [{}] {}", self.id, if self.removed { "[REMOVED]" } else { "" }, if self.deleted { "[DELETED]" } else { "" }, self.published, self.name)
    }
}
