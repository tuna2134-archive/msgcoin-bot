use std::env;
use std::sync::Arc;

use serenity::async_trait;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, StandardFramework};
use serenity::model::{channel::Message, gateway::Ready, guild::Member, id::ChannelId};
use serenity::prelude::*;
use sqlx::mysql::MySqlPool;
use tokio::sync::Mutex;

use dotenv::dotenv;

struct PoolContainer;

impl TypeMapKey for PoolContainer {
    type Value = Arc<Mutex<MySqlPool>>;
}

#[group]
#[commands(ping, check)]
struct General;

struct Handler;

async fn get_pool(ctx: &Context) -> MySqlPool {
    let data = ctx.data.read().await;
    let pool_locked = data.get::<PoolContainer>().unwrap();
    let pool = pool_locked.lock().await;
    pool.clone()
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        log::info!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let pool = get_pool(&ctx).await;
        let recs = sqlx::query!(
            "SELECT Point, Level FROM User WHERE UserId = ?",
            msg.author.id.0
        )
        .fetch_one(&pool)
        .await;
        match recs {
            Ok(rec) => {
                let point = rec.Point.unwrap() + 1;
                let mut level = rec.Level.unwrap();
                if point >= level * 10 {
                    level += 1;
                    msg.reply(ctx, format!("あなたはレベル{}になりました。", level))
                        .await
                        .unwrap();
                    sqlx::query!(
                        "UPDATE User SET Level = ?, Point = ? WHERE Userid = ?",
                        level,
                        0,
                        msg.author.id.0
                    )
                    .execute(&pool)
                    .await
                    .unwrap();
                } else {
                    sqlx::query!(
                        "UPDATE User SET Point = ? WHERE UserId = ?",
                        point,
                        msg.author.id.0
                    )
                    .execute(&pool)
                    .await
                    .unwrap();
                }
            }
            Err(_) => {
                sqlx::query!(
                    "INSERT INTO User(UserId, Point, Level) VALUES(?, ?, ?)",
                    msg.author.id.0,
                    0,
                    1
                )
                .execute(&pool)
                .await
                .unwrap();
            }
        }
    }
    async fn guild_member_addition(&self, ctx: Context, member: Member) {
        let pool = get_pool(&ctx).await;
        let recs = sqlx::query!("SELECT * FROM Welcome WHERE GuildId = ?", member.guild_id.0)
            .fetch_one(&pool)
            .await;
        match recs {
            Ok(rec) => {
                let channel_id = rec.ChannelId.unwrap();
                let channel = ChannelId(channel_id as u64);
                let channel = ctx.cache.guild_channel(channel);
                match channel {
                    Some(channel) => {
                        channel.say(ctx, rec.Message.unwrap()).await.unwrap();
                    }
                    None => {
                        log::error!("チャンネルが見つかりませんでした。");
                    }
                }
            }
            Err(_) => {
                log::error!("データがないぞい");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    dotenv().ok();
    let pool = MySqlPool::connect(&env::var("DATABASE_URL").unwrap())
        .await
        .expect("Ok");
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~")) // set the bot's prefix to "~"
        .group(&GENERAL_GROUP);

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");
    {
        let mut data = client.data.write().await;
        data.insert::<PoolContainer>(Arc::new(Mutex::new(pool)));
    }
    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}

#[command]
async fn check(ctx: &Context, msg: &Message) -> CommandResult {
    let pool = get_pool(ctx).await;
    let recs = sqlx::query!("SELECT Level FROM User WHERE UserId = ?", msg.author.id.0)
        .fetch_one(&pool)
        .await;
    match recs {
        Ok(rec) => {
            let level = rec.Level.unwrap();
            msg.reply(ctx, format!("あなたのレベルは{}です。", level))
                .await?;
        }
        Err(_) => {
            msg.reply(ctx, "データがありません").await?;
        }
    }
    Ok(())
}
