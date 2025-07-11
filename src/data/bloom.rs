use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use anyhow::{Error, Result};
use poise::Context as PoiseContext;
use rand::{SeedableRng, rngs::SmallRng};
use reqwest::Client;
use tokio::sync::Mutex;

use crate::database::DatabaseHandler;
use crate::handlers::embeddings::OpenAIHandler;

pub struct MinimalCommand {
  pub name: String,
  pub id: u64,
}

pub struct Data {
  pub db: Arc<DatabaseHandler>,
  pub rng: Arc<Mutex<SmallRng>>,
  pub embeddings: Arc<OpenAIHandler>,
  pub bloom_start_time: Instant,
  pub term_names: Arc<RwLock<Vec<String>>>,
  pub http: Client,
  pub voice_state: Arc<Mutex<HashMap<u64, Instant>>>,
  pub commands: Arc<Vec<MinimalCommand>>,
}

pub type Context<'a> = PoiseContext<'a, Data, Error>;

impl Data {
  pub fn new(
    db: DatabaseHandler,
    term_names: Vec<String>,
    commands: Vec<MinimalCommand>,
  ) -> Result<Self> {
    Ok(Self {
      db: Arc::new(db),
      rng: Arc::new(Mutex::new(SmallRng::from_os_rng())),
      embeddings: Arc::new(OpenAIHandler::new()?),
      bloom_start_time: Instant::now(),
      term_names: Arc::new(RwLock::new(term_names)),
      http: Client::new(),
      voice_state: Arc::new(Mutex::new(HashMap::new())),
      commands: Arc::new(commands),
    })
  }
}
