use anyhow::anyhow;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use crate::comment::Comment;
use crate::configuration::Configuration;
use crate::post::Post;

/// An API client for Lemmy. Quite barebones.
pub(crate) struct Api {
    base_url: String,
    pub(crate) client: Client,
}

impl Api {
    /// Generate the URL for a Lemmy API endpoint.
    pub fn format_api_call(&self, path: &str) -> String {
        format!("{}/api/v3/{path}", self.base_url)
    }

    /// Build a reqwest client. Used for initialisation.
    fn build_client() -> Client {
        ClientBuilder::new()
            .user_agent("LemmyAutoDeleteBot/0.1.0")
            .build()
            .unwrap()
    }
}

impl Default for Api {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            client: Self::build_client(),
        }
    }
}

impl TryFrom<&Configuration> for Api {
    type Error = anyhow::Error;

    fn try_from(value: &Configuration) -> anyhow::Result<Self> {
        let (_user, domain) = value.canonical_username()
            .split_once('@')
            .ok_or(anyhow!("Invalid username"))?;

        Ok(Self {
            base_url: format!("https://{domain}"),
            client: Self::build_client(),
        })
    }
}

/// A struct representing comments on a profile. Simplified.
#[derive(Deserialize)]
pub(crate) struct CommentView {
    /// The comment details itself.
    pub(crate) comment: Comment,
    /// Whether the comment has been saved by the user or not.
    pub(crate) saved: bool,
    /// What vote the user gave to this comment (1, 0, -1)
    pub(crate) my_vote: Option<i64>,
}

/// A struct representing the response to an edit API call. Simplified.
#[derive(Deserialize)]
pub(crate) struct CommentEditResponse {
    pub(crate) comment_view: CommentView,
}

/// A struct representing the response to a post delete API call. Simplified.
#[derive(Deserialize)]
pub(crate) struct PostDeleteResponse {
    pub(crate) post_view: PostView,
}

/// A struct representing the posts on a profile. Simplified.
#[derive(Deserialize)]
pub(crate) struct PostView {
    pub(crate) post: Post,
    pub(crate) saved: bool,
    pub(crate) my_vote: Option<i64>,
    pub(crate) deleted: Option<bool>,
}

/// A struct representing a single page of a profile. Simplified.
#[derive(Deserialize)]
pub(crate) struct ProfilePage {
    /// The comments for this page. This list is not complete, there may be more pages.
    pub(crate) comments: Vec<CommentView>,
    /// The posts for this page. This list is not complete, there may be more pages.
    pub(crate) posts: Vec<PostView>,
}

/// A struct for making API calls that take a post ID and a delete flag.
#[derive(Serialize)]
pub(crate) struct PostIdBody {
    /// The Lemmy auth token
    pub(crate) auth: String,
    /// The post ID to delete
    pub(crate) post_id: i64,
    /// Whether to delete or not (should probably be true)
    pub(crate) deleted: bool,
}

impl PostIdBody {
    pub fn new(post_id: i64, auth: String) -> Self { Self { post_id, deleted: true, auth } }
}

/// A struct for building the body for a comment delete API call
#[derive(Serialize)]
pub(crate) struct DeleteCommentBody {
    /// The Lemmy auth token
    pub(crate) auth: String,
    /// The comment ID to delete
    pub(crate) comment_id: i64,
    /// Whether to delete or not (should probably be true)
    pub(crate) deleted: bool,
}


impl DeleteCommentBody {
    pub(crate) fn new(source: &Comment, configuration: &Configuration) -> Self {
        Self {
            auth: configuration.lemmy_token.clone(),
            comment_id: source.id,
            deleted: true,
        }
    }
}

/// A struct for building the body for a comment edit API call
#[derive(Serialize)]
pub(crate) struct EditCommentBody {
    /// The Lemmy auth token
    pub(crate) auth: String,
    /// The ID of the comment to edit
    pub(crate) comment_id: i64,
    /// The contents that should replace the current contents of the comment. Markdown.
    pub(crate) content: String,
}

impl EditCommentBody {
    pub fn new(source: &Comment, config: &Configuration) -> Self {
        Self {
            auth: config.lemmy_token.clone(),
            comment_id: source.id,
            content: config.encoded_edit_text().to_string(),
        }
    }
}