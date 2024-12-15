use std::env;
use teloxide::{prelude::*, utils::command::BotCommands};
use sqlx::{PgPool, Row};
use dotenv::dotenv;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These are the available commands:"
)]
enum Command {
    #[command(description = "Submit a new proposal")]
    Submit(String),
    #[command(description = "View all proposals")]
    View,
    #[command(description = "Vote for a proposal")]
    Vote { id: i32 },
    #[command(description = "Show help")]
    Help,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Load environment variables
    let bot_token = env::var("BOT_TOKEN").expect("BOT_TOKEN must be set");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create a PostgreSQL connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to create database pool");

    // Create bot instance
    let bot = Bot::new(bot_token);

    teloxide::commands_repl(bot, Command::ty(), move |cx, command| {
        let pool = pool.clone();
        async move {
            match command {
                Command::Submit(text) => submit_proposal(cx, &pool, text).await,
                Command::View => view_proposals(cx, &pool).await,
                Command::Vote { id } => vote_proposal(cx, &pool, id).await,
                Command::Help => cx.answer(Command::descriptions()).send().await?,
            };
            Ok(())
        }
    })
    .await;
}

async fn submit_proposal(cx: UpdateWithCx<Message>, pool: &PgPool, text: String) -> Result<(), teloxide::RequestError> {
    let user_id = cx.update.from().map(|user| user.id).unwrap_or_default();
    let username = cx.update.from().map(|user| user.username.clone()).flatten().unwrap_or_default();

    sqlx::query!(
        "INSERT INTO proposals (user_id, username, text, votes) VALUES ($1, $2, $3, 0)",
        user_id as i64,
        username,
        text
    )
    .execute(pool)
    .await
    .expect("Failed to insert proposal");

    cx.answer(format!("Proposal submitted: {}", text)).send().await?;
    Ok(())
}

async fn view_proposals(cx: UpdateWithCx<Message>, pool: &PgPool) -> Result<(), teloxide::RequestError> {
    let rows = sqlx::query("SELECT id, username, text, votes FROM proposals ORDER BY votes DESC")
        .fetch_all(pool)
        .await
        .expect("Failed to fetch proposals");

    let mut response = String::from("Proposals:\n");
    for row in rows {
        let id: i32 = row.get("id");
        let username: String = row.get("username");
        let text: String = row.get("text");
        let votes: i32 = row.get("votes");
        response.push_str(&format!("ID: {} | By: {} | Votes: {}\n{}\n\n", id, username, votes, text));
    }

    cx.answer(response).send().await?;
    Ok(())
}

async fn vote_proposal(cx: UpdateWithCx<Message>, pool: &PgPool, id: i32) -> Result<(), teloxide::RequestError> {
    let result = sqlx::query!(
        "UPDATE proposals SET votes = votes + 1 WHERE id = $1 RETURNING text",
        id
    )
    .fetch_one(pool)
    .await;

    match result {
        Ok(row) => {
            let proposal_text = row.get::<String, _>("text");
            cx.answer(format!("You voted for proposal: {}", proposal_text))
                .send()
                .await?;
        }
        Err(_) => {
            cx.answer("Invalid proposal ID").send().await?;
        }
    }

    Ok(())
}
