use anyhow::Result;
use dotenv::dotenv;
use dotenv::var;
use sqlx::SqlitePool;


use warp::cors::Cors;

pub mod db_functions;
pub mod password_auth;
pub mod routes;

//pub type ArcState = Arc<Mutex<State>>;

#[derive(Clone)]
pub struct State {
    db: SqlitePool,
    cors: Cors,
}

impl State {
    pub async fn init() -> Self {
        let db = make_db_pool().await.unwrap();
        let cors = make_cors();
        Self { db, cors }
    }
    pub fn db(&self) -> &SqlitePool {
        &self.db
    }
    pub fn cors(&self) -> &Cors {
        &self.cors
    }
}

pub fn make_cors() -> Cors {
    
    warp::cors().allow_any_origin().build()
}

pub async fn make_db_pool() -> Result<SqlitePool> {
    dotenv()?;

    let pool = SqlitePool::connect(&var("DATABASE_URL")?).await.unwrap();
    Ok(pool)
}
