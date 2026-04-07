//! One-shot helper for stress testing: inserts an active IMAP
//! connection row for a given user using the backend's existing
//! encryption, so the next backend restart will spawn an IDLE task
//! for it via `initialize_all_idle_tasks`.
//!
//! Usage:
//!   cargo run --bin add_imap_connection -- \
//!       --user-id 1 \
//!       --email rasmus@lightfriend.ai \
//!       --password '...' \
//!       --host mail.privateemail.com \
//!       --port 993
//!
//! Safe to re-run: it upserts by (user_id, email).

use clap::Parser;
use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use std::env;

#[derive(Parser, Debug)]
#[command(about = "Add an IMAP connection to the lightfriend PG database")]
struct Args {
    #[arg(long)]
    user_id: i32,

    #[arg(long)]
    email: String,

    #[arg(long)]
    password: String,

    #[arg(long, default_value = "mail.privateemail.com")]
    host: String,

    #[arg(long, default_value_t = 993)]
    port: u16,
}

fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    let pg_url = env::var("PG_DATABASE_URL").expect("PG_DATABASE_URL must be set");
    // Sanity: ENCRYPTION_KEY must be present because set_imap_credentials
    // will use it via the encrypt() helper.
    env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set (from .env)");

    let manager = ConnectionManager::<PgConnection>::new(&pg_url);
    let pool: backend::PgDbPool = r2d2::Pool::builder()
        .max_size(2)
        .build(manager)
        .expect("Failed to build PG pool");

    let user_repo = backend::UserRepository::new(pool);

    let id = user_repo
        .set_imap_credentials(
            args.user_id,
            &args.email,
            &args.password,
            Some(&args.host),
            Some(args.port),
        )
        .expect("Failed to set IMAP credentials");

    println!(
        "OK: imap_connection id={} user_id={} email={} host={}:{}",
        id, args.user_id, args.email, args.host, args.port
    );
    println!("Restart the backend to have initialize_all_idle_tasks spawn an IDLE task for it.");
}
