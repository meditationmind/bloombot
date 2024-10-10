# Bloom Bot

Bloom Bot is a Discord bot that allows for many commands seen in the Meditation Mind server. It 
is built in Rust using the [Poise](https://docs.rs/poise/0.6.1/poise/index.html) library (built on top of Serenity) for Discord bot features.

## Setup

1. Install Rust
2. Clone the repository
3. Run PostgreSQL: `docker run --name bloom-db -e POSTGRES_PASSWORD=supersecret -p 5432:5432 -d postgres` (choose any password you would like)
4. Copy the `.env.example` file to `.env` and fill in the necessary values. Be sure to set the password in the DATABASE_URL to the one you chose in step 3.
5. Run `cargo run` to start the bot

