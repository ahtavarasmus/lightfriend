pub mod config;
pub mod db;
pub mod handlers;
pub mod proxy;

pub use config::Config;
pub use db::{Database, EnclaveTarget};

pub struct AppState {
    pub db: Database,
    pub config: Config,
}
