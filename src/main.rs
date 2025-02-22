use clap::Parser;

mod configuration;
mod api;
mod comment;
mod helper;
mod post;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Days, Utc};
use crate::configuration::Configuration;
use crate::api::{Api, CommentEditResponse, DeleteCommentBody, EditCommentBody, PostDeleteResponse, PostIdBody, ProfilePage};
use crate::comment::Comment;
use crate::post::Post;

/// Check if a date-time is within a certain date threshold
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

/// Pre-collect all data from the profile page API
/// This could probably be written as an iterator to be more efficient, but for simplicity we fetch
/// all of this before doing any modifications.
///
/// This will either result two vectors (comments and posts) or an error indicating why these
/// vectors couldn't be retrieved.
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

/// Delete a post.
///
/// It will return `Ok(true)` for deletes than have been requested successfully, `Ok(false)` for
/// deletes that have been requested but that the server did not flag as deleted in the response,
/// and anything else to indicate a general error.
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


/// Edit a comment to replace its contents
///
/// This method will either return `Ok(true)` to indicate that the edit was successful, `Ok(false)`
/// to indicate that the edit was successfully requested but the server did not apply the change,
/// or anything else to indicate an error occurred.
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


/// Delete a comment.
///
/// It will return `Ok(true)` for deletes than have been requested successfully, `Ok(false)` for
/// deletes that have been requested but that the server did not flag as deleted in the response,
/// and anything else to indicate a general error.
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
