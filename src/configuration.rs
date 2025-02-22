use std::time::Duration;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(author = "Skull Giver", version, about = "Automatically delete old comments and posts", long_about = None)]
pub(crate) struct Configuration {
    #[arg(long, env)]
    pub(crate) username: String,
    #[arg(short, long, env)]
    pub(crate) lemmy_token: String,
    #[arg(short = 'k', long, env, default_value = "14")]
    pub(crate) days_to_keep: u64,
    #[arg(short = 'f', long, env, default_value = "false")]
    pub(crate) keep_favourites: bool,
    #[arg(short = 'u', long, env, default_value = "false")]
    pub(crate) keep_upvotes: bool,
    #[arg(short = 'd', long, env, default_value = "false")]
    pub(crate) keep_downvotes: bool,
    #[arg(short = 'e', long, env, default_value = "true")]
    pub(crate) edit_then_delete: bool,
    #[arg(short = 't', long, env, default_value = "[This comment has been deleted by an automated system]")]
    pub(crate) edit_text: String,
    #[arg(short = 'w', long, env, default_value = "100")]
    pub(crate) sleep_time: u64,
}

impl Configuration {
    /// Turn a username, as passed in the configuration, into something usable for the API.
    pub fn canonical_username(&self) -> &str {
        if self.username.starts_with('@') {
            &self.username[1..]
        } else {
            &self.username[..]
        }
    }

    /// Helper function that will pre-process a comment edit.
    /// Kept here in case Lemmy bugs need working around.
    pub fn encoded_edit_text(&self) -> &str {
        &self.edit_text[..]
    }

    /// Wait for the configured sleep time. Used in between API calls.
    pub(crate) async fn wait(&self) {
        tokio::time::sleep(Duration::from_millis(self.sleep_time)).await
    }

    /// Wait for the server to recover (rate limits, overwhelmed server).
    /// This will wait ten times the configured sleep time.
    pub(crate) async fn wait_for_recovery(&self) {
        for _ in 0..10 {
            self.wait().await;
        }
    }

    pub(crate) fn auth_header(&self) -> String {
        format!("Bearer {}", self.lemmy_token)
    }
}