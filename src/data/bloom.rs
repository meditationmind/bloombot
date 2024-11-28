use std::sync::{Arc, RwLock};
use std::time::Instant;

use anyhow::{Error, Result};
use poise::Context as PoiseContext;
use rand::{rngs::SmallRng, SeedableRng};
use tokio::sync::Mutex;

use crate::database::DatabaseHandler;
use crate::handlers::embeddings::OpenAIHandler;

pub struct Data {
  pub db: Arc<DatabaseHandler>,
  pub rng: Arc<Mutex<SmallRng>>,
  pub embeddings: Arc<OpenAIHandler>,
  pub bloom_start_time: Instant,
  pub term_names: Arc<RwLock<Vec<String>>>,
}

pub type Context<'a> = PoiseContext<'a, Data, Error>;

impl Data {
  pub fn new(db: DatabaseHandler, term_names: Vec<String>) -> Result<Self> {
    Ok(Self {
      db: Arc::new(db),
      rng: Arc::new(Mutex::new(SmallRng::from_entropy())),
      embeddings: Arc::new(OpenAIHandler::new()?),
      bloom_start_time: Instant::now(),
      term_names: Arc::new(RwLock::new(term_names)),
    })
  }
}
