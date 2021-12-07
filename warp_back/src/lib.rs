use anyhow::Result;
use dotenv::dotenv;
use dotenv::var;
use sqlx::SqlitePool;

use warp::cors::Cors;

pub mod db_functions;
pub mod error_handling;
pub mod password_auth;
pub mod routes;
pub mod test_stuff;

pub const HOME_ORIGIN: &str = "http://192.168.137.107:8080";

#[derive(Clone)]
pub struct State {
    db: SqlitePool,
    cors: Cors,
}

impl State {
    pub async fn init() -> Result<Self> {
        let db = make_db_pool().await?;
        let cors = make_cors();
        Ok(Self { db, cors })
    }
    pub fn db(&self) -> &SqlitePool {
        &self.db
    }
    pub fn cors(&self) -> &Cors {
        &self.cors
    }
}

pub fn make_cors() -> Cors {
    warp::cors()
        .allow_headers(vec!["Content-Type"])
        .allow_methods(&[
            warp::http::Method::GET,
            warp::http::Method::POST,
            warp::http::Method::OPTIONS,
        ])
        .allow_origin(HOME_ORIGIN)
        .build()
}

pub async fn make_db_pool() -> Result<SqlitePool> {
    dotenv()?;

    let pool = SqlitePool::connect(&var("DATABASE_URL")?).await?;
    Ok(pool)
}
