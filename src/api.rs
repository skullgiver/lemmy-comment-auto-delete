use anyhow::anyhow;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use crate::comment::Comment;
use crate::configuration::Configuration;
use crate::post::Post;

pub(crate) struct Api {
    base_url: String,
    pub(crate) client: Client,
}

impl Api {
    pub fn format_api_call(&self, path: &str) -> String {
        format!("{}/api/v3/{path}", self.base_url)
    }


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


#[derive(Deserialize)]
pub(crate) struct CommentView {
    pub(crate) comment: Comment,
    pub(crate) saved: bool,
    pub(crate) my_vote: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct CommentEditResponse {
    pub(crate) comment_view: CommentView,
}

#[derive(Deserialize)]
pub(crate) struct PostDeleteResponse {
    pub(crate) post_view: PostView,
}

#[derive(Deserialize)]
pub(crate) struct PostView {
    pub(crate) post: Post,
    pub(crate) saved: bool,
    pub(crate) my_vote: Option<i64>,
    pub(crate) deleted: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct ProfilePage {
    pub(crate) comments: Vec<CommentView>,
    pub(crate) posts: Vec<PostView>,
}


#[derive(Serialize)]
pub(crate) struct PostIdBody {
    pub(crate) auth: String,
    pub(crate) post_id: i64,
    pub(crate) deleted: bool,
}

impl PostIdBody {
    pub fn new(post_id: i64, auth: String) -> Self { Self { post_id, deleted: true, auth } }
}

#[derive(Serialize)]
pub(crate) struct DeleteCommentBody {
    pub(crate) auth: String,
    pub(crate) comment_id: i64,
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