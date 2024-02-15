use std::env;
use std::error::Error;

use dotenv;
use futures::{self, StreamExt};
use regex::Regex;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler;

fn is_short_url(url: &str) -> bool {
    let re = Regex::new(r"^https://vm\.tiktok\.com/.+$").unwrap();
    re.is_match(url)
}

fn is_long_url(url: &str) -> bool {
    let re = Regex::new(r"^https://www\.tiktok\.com/@[^/]+/video/\d+").unwrap();
    re.is_match(url)
}

fn replace_long_url(url: &str) -> String {
    let re = Regex::new(r"https://www\.tiktok\.com").unwrap();
    re.replace(url, "https://www.vxtiktok.com").to_string()
}

fn extract_urls(text: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let url_regex = r"\bhttps?://[^\s()<>]+(?:\([\w\d]+\)|([^[:punct:]\s]|/))";
    let re = Regex::new(url_regex)?;

    let urls = re
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect::<Vec<String>>();

    Ok(urls)
}

async fn get_long_url_from_short_url(url: &str) -> Option<String> {
    let client = reqwest::Client::new();
    if !is_short_url(url) {
        return None;
    }

    let res = match client.head(url).send().await {
        Ok(res) => res,
        Err(why) => {
            println!("Error sending request: {:?}", why);
            return None;
        }
    };

    let long_url = res.url().as_str().to_string();
    if !is_long_url(long_url.as_str()) {
        if let Some(location) = res.headers().get("location") {
            match location.to_str() {
                Ok(location) => {
                    if is_long_url(location) {
                        return Some(replace_long_url(location));
                    } else {
                        return None;
                    }
                }
                Err(why) => {
                    println!("Error getting location: {:?}", why);
                    return None;
                }
            }
        }
    }

    Some(replace_long_url(&long_url))
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let urls = match extract_urls(&msg.content) {
            Ok(urls) => urls,
            Err(_) => return,
        };

        let http = ctx.http.clone();
        let futures = futures::stream::iter(urls.into_iter().map(move |url| {
            let http = http.clone();
            async move {
                if is_short_url(&url) {
                    if let Some(long_url) = get_long_url_from_short_url(&url).await {
                        let _ = msg.channel_id.say(&http, long_url).await;
                    }
                } else if is_long_url(&url) {
                    let new_url = replace_long_url(&url);
                    let _ = msg.channel_id.say(&http, new_url).await;
                }
            }
        }))
        .buffer_unordered(4)
        .collect::<Vec<()>>();

        futures.await;
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
