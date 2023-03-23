use std::env;
use std::sync::Arc;

use serenity::async_trait;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, StandardFramework};
use serenity::model::{
    channel::Message,
    gateway::{Ready},
};
use serenity::prelude::*;
use sqlx::mysql::MySqlPool;
use tokio::sync::Mutex;

use dotenv::dotenv;

struct PoolContainer;

impl TypeMapKey for PoolContainer {
    type Value = Arc<Mutex<MySqlPool>>;
}

#[group]
#[commands(ping)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        log::info!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let data = ctx.data.read().await;
        let pool = data.get::<PoolContainer>().unwrap();
        let mut pool = pool.lock().await;
        sqlx::query!("SELECT Point FROM Point WHERE UserId = ?", msg.author.id.0)
            .fetch_one(&mut *pool)
            .await
            .unwrap();
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    dotenv().ok();
    let pool = MySqlPool::connect(&env::var("DATABASE_URL").unwrap())
        .await
        .expect("Ok");
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
