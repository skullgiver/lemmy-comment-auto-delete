use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::time::Duration;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Days, NaiveDateTime, Utc};
use clap::{Parser};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Deserializer, Serialize};

struct TryAgainError {}

impl Debug for TryAgainError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Application requested to try again later")
    }
}

impl Display for TryAgainError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for TryAgainError {}

#[derive(Debug, Parser)]
#[command(author = "Skull Giver", version, about = "Automatically delete old comments and posts", long_about = None)]
struct Configuration {
    #[arg(long, env)]
    username: String,
    #[arg(short, long, env)]
    lemmy_token: String,
    #[arg(short = 'k', long, env, default_value = "14")]
    days_to_keep: u64,
    #[arg(short = 'f', long, env, default_value = "false")]
    keep_favourites: bool,
    #[arg(short = 'u', long, env, default_value = "false")]
    keep_upvotes: bool,
    #[arg(short = 'd', long, env, default_value = "false")]
    keep_downvotes: bool,
    #[arg(short = 'e', long, env, default_value = "true")]
    edit_then_delete: bool,
    #[arg(short = 't', long, env, default_value = "[This comment has been deleted by an automated system]")]
    edit_text: String,
    #[arg(short = 'w', long, env, default_value = "100")]
    sleep_time: u64,
}

impl Configuration {
    pub fn canonical_username(&self) -> &str {
        if self.username.starts_with('@') {
            &self.username[1..]
        } else {
            &self.username[..]
        }
    }

    pub fn encoded_edit_text(&self) -> &str {
        &self.edit_text[..]
    }

    async fn wait(&self) {
        tokio::time::sleep(Duration::from_millis(self.sleep_time)).await
    }

    async fn wait_for_recovery(&self) {
        for _ in 0..10 {
            self.wait().await;
        }
    }
}

struct Api {
    base_url: String,
    client: Client,
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

    fn try_from(value: &Configuration) -> Result<Self> {
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
struct ProfilePage {
    comments: Vec<CommentView>,
    posts: Vec<PostView>,
}

// You can use this deserializer for any type that implements FromStr
// and the FromStr::Err implements Display
fn deserialize_date<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let format = "%Y-%m-%dT%H:%M:%S%.6f";
    NaiveDateTime::parse_and_remainder(&s, format)
        .map(|(local_date, _remainder)| {
            DateTime::from_naive_utc_and_offset(local_date, Utc)
        })
        .map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
struct Comment {
    id: i64,
    content: String,
    removed: bool,
    deleted: Option<bool>,
    #[serde(deserialize_with = "deserialize_date")]
    published: DateTime<Utc>,
}

impl Comment {
    pub fn item_id(&self) -> String {
        format!("{}", self.id)
    }
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

#[derive(Deserialize)]
struct Counts {
    score: i64,
    upvotes: i64,
    downvotes: i64,
}

impl Display for Counts {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (+{}, -{})", self.score, self.upvotes, self.downvotes)
    }
}

#[derive(Deserialize)]
struct CommentView {
    comment: Comment,
    saved: bool,
    my_vote: Option<i64>,
}

#[derive(Deserialize)]
struct CommentEditResponse {
    comment_view: CommentView,
}

#[derive(Deserialize)]
struct PostDeleteResponse {
    post_view: PostView,
}

#[derive(Deserialize)]
struct PostView {
    post: Post,
    saved: bool,
    my_vote: Option<i64>,
    deleted: Option<bool>,
}

pub fn within_days(date: DateTime<Utc>, days: u64) -> bool {
    match date.checked_add_days(Days::new(days)) {
        Some(past_datestamp) => {
            let now = Utc::now();
            past_datestamp > now
        }
        None => {
            // Safe fallback, do not delete
            eprintln!("Couldn't add 7 days to {}", date);
            true
        }
    }
}

#[derive(Deserialize)]
struct Post {
    id: i64,
    name: String,
    removed: bool,
    deleted: bool,
    #[serde(deserialize_with = "deserialize_date")]
    published: DateTime<Utc>,
}

impl Post {
    pub fn item_id(&self) -> String {
        format!("{}", self.id)
    }
}

impl Display for Post {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Post {}{}{}: [{}] {}", self.id, if self.removed { "[REMOVED]" } else { "" }, if self.deleted { "[DELETED]" } else { "" }, self.published, self.name)
    }
}


async fn gather_data_from_profile(config: &Configuration) -> Result<(Vec<Comment>, Vec<Post>)> {
    let api: Api = config.try_into()?;

    let mut comments = vec![];
    let mut posts = vec![];

    let mut page = 1;
    loop {
        println!("Fetching comments, page {page}");
        let fetch_path = api.format_api_call(&format!("user?username={username}&sort=Old&page={page}&limit=50&auth={auth}", username = config.canonical_username(), auth = config.lemmy_token));

        let results = match api.client
            .get(&fetch_path)
            .send().await {
            Ok(response) => response,
            Err(error) => {
                Err(error)?
            }
        };
        if !results.status().is_success() {
            eprintln!("HTTP error ({}), impending error!", results.status());
        }

        let results: ProfilePage = match results.json().await {
            Ok(results) => results,
            Err(error) => {
                eprintln!("Aborting loop because of error: {error}");
                break;
            }
        };

        if results.comments.is_empty() && results.posts.is_empty() {
            break;
        }

        for comment in results.comments {
            // Skip deleted comments
            if comment.comment.deleted == Some(true) {
                continue;
            }

            // Skip upvotes if enabled
            if config.keep_upvotes && comment.my_vote.filter(|&i| i > 0).is_some() {
                continue;
            }
            // Skip downvotes if enabled
            if config.keep_downvotes && comment.my_vote.filter(|&i| i < 0).is_some() {
                continue;
            }

            // Keep saved
            if config.keep_favourites && comment.saved {
                continue;
            }

            // Stick to provided day limit
            if within_days(comment.comment.published, config.days_to_keep) {
                continue;
            }

            comments.push(comment.comment);
        }

        for post in results.posts {
            // Skip deleted posts
            if post.deleted == Some(true) || post.post.deleted {
                continue;
            }

            // Keep upvoted posts
            if config.keep_upvotes && post.my_vote.filter(|&i| i > 0).is_some() {
                continue;
            }

            // Keep downvoted posts
            if config.keep_downvotes && post.my_vote.filter(|&i| i < 0).is_some() {
                continue;
            }

            // Keep favourites
            if config.keep_favourites && post.saved {
                continue;
            }

            // Stick to provided day limit
            if within_days(post.post.published, config.days_to_keep) {
                continue;
            }

            posts.push(post.post);
        }

        page += 1;

        config.wait().await;
    }

    Ok((comments, posts))
}

#[derive(Serialize)]
struct PostIdBody {
    auth: String,
    post_id: i64,
    deleted: bool,
}

impl PostIdBody {
    pub fn new(post_id: i64, auth: String) -> Self { Self { post_id, deleted: true, auth } }
}

async fn delete_post(config: &Configuration, post: &Post) -> Result<bool> {
    if post.deleted {
        println!("BUG: request to delete deleted post");
        return Ok(true);
    }

    config.wait().await;

    let api: Api = config.try_into()?;

    let url = api.format_api_call("post/delete");

    let request = match api.client.post(url).json(&PostIdBody::new(post.id, config.lemmy_token.clone())).send().await {
        Ok(ok) => ok,
        Err(err) => Err(err)?
    };

    if !request.status().is_success() {
        eprintln!("Delete failure for comment {}: {}", post.item_id(), request.status());
    }

    let response = request.text().await?;

    let response: PostDeleteResponse = match serde_json::from_str(&response) {
        Ok(response) => response,
        Err(err) => {
            eprintln!("Post delete parse failure: {err}");
            eprintln!("Input that failed to parse was: {response}");

            Err(err)?
        }
    };

    response.post_view.deleted.ok_or(anyhow!("Failed to verify deletion"))
}

#[derive(Serialize)]
struct EditCommentBody {
    auth: String,
    comment_id: i64,
    content: String,
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


async fn edit_comment(config: &Configuration, comment: &Comment) -> Result<bool> {
    if comment.deleted == Some(true) {
        println!("Bug: request to edit deleted comment");
        return Ok(true);
    }

    let api: Api = config.try_into()?;

    let url = api.format_api_call("comment");

    let mut tries = 3;

    while tries > 0 {
        let request = match api.client
            .put(&url)
            .json(&EditCommentBody::new(comment, config))
            .send()
            .await {
            Ok(ok) => ok,
            Err(err) => Err(err)?
        };

        let status_code = request.status();
        if !status_code.is_success() {
            // Ignore acceptable errors
            if status_code.as_u16() == 503 {
                // Server is overwhelmed
                config.wait_for_recovery().await;
                tries -= 1;
                continue;
            }

            let body = request.text().await?;
            eprintln!("Edit failure for comment {}: {}", comment.item_id(), status_code);
            eprintln!("Edit result body: {body}");

            return Err(anyhow!("Edit failure for comment {}: {}", comment.item_id(), status_code));
        }

        let response: CommentEditResponse = request.json().await?;

        return if response.comment_view.comment.content != config.encoded_edit_text() {
            Err(anyhow!("Edit did not succeed"))
        } else {
            Ok(true)
        };
    }

    Err(anyhow!("Failure"))
}


#[derive(Serialize)]
struct DeleteCommentBody {
    auth: String,
    comment_id: i64,
    deleted: bool,
}

impl DeleteCommentBody {
    fn new(source: &Comment, configuration: &Configuration) -> Self {
        Self {
            auth: configuration.lemmy_token.clone(),
            comment_id: source.id,
            deleted: true,
        }
    }
}

async fn delete_comment(config: &Configuration, comment: &Comment) -> Result<bool> {
    if comment.deleted == Some(true) {
        println!("Bug: tried to delete a deleted comment");
        return Ok(true);
    }

    config.wait().await;

    if config.edit_then_delete {
        edit_comment(config, comment).await?;
    }

    let api: Api = config.try_into()?;

    let url = api.format_api_call("comment/delete");

    let mut tries = 3;

    while tries > 0 {
        let request = match api.client.post(&url)
            .json(&DeleteCommentBody::new(comment, config))
            .send().await {
            Ok(ok) => ok,
            Err(err) => Err(err)?
        };

        if !request.status().is_success() {
            eprintln!("Delete failure for comment {}: {}", comment.item_id(), request.status());

            // Acceptable error, retry
            if request.status().as_u16() == 503 {
                config.wait_for_recovery().await;
                tries -= 1;
                continue;
            }
        }

        let response_text = request.text().await?;

        let response: CommentEditResponse = match serde_json::from_str(&response_text) {
            Ok(response) => response,
            Err(err) => {
                eprintln!("Comment deletion parse failure: {err}");
                eprintln!("Output failed to parse: {response_text}");
                Err(err)?
            }
        };

        return response.comment_view.comment.deleted.ok_or(anyhow!("Failed to verify deletion"));
    }


    Err(anyhow!("Too many failed tries, giving up on comment {}", comment.id))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Configuration::parse();

    println!("Hello, {username}, after this program succeeds you should only have {duration} days of comments and posts left", username = config.canonical_username(), duration = config.days_to_keep);
    if config.keep_favourites {
        println!(" + Items you've favourited will also be kept");
    }
    if config.keep_upvotes {
        println!(" + Upvotes will also be kept");
    }
    if config.keep_downvotes {
        println!(" + Downvotes will also be kept");
    }
    if config.edit_then_delete {
        println!(" + Comments will first be edited into the string '{}'", config.edit_text);
    }

    let (comments, posts) = gather_data_from_profile(&config).await?;

    for post in posts {
        match delete_post(&config, &post).await {
            Ok(delete_respected) => {
                println!("Delete for post{} respected: {post}", if delete_respected { "" } else { " NOT" });
            }
            Err(error) => eprintln!("Deletion request failed for post {}: {error}", post.item_id())
        }
    }

    for comment in comments {
        match delete_comment(&config, &comment).await {
            Ok(delete_respected) => {
                println!("Delete for comment{} respected: {comment}", if delete_respected { "" } else { " NOT" });
            }
            Err(error) => {
                eprintln!("Deletion request failed for comment {}: {error}", comment.item_id())
            }
        }
    }

    Ok(())
}
