use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "backend")]
#[command(about = "Lightfriend backend CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Parser, Debug)]
pub enum Commands {}

pub async fn run_cli() -> Result<bool, Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        None => Ok(false),
    }
}
