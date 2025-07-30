//! # FHIRSchema CLI
//!
//! Command-line interface for FHIRSchema tools.

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber;

mod commands;

use commands::{convert::ConvertCommand, validate::ValidateCommand, download::DownloadCommand, completion::CompletionCommand, /* repository::RepositoryCommand, */ codegen::CodegenCommand};

#[derive(Parser)]
#[command(name = "fhirschema")]
#[command(about = "A CLI tool for working with FHIRSchema")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert FHIR StructureDefinition to FHIRSchema
    Convert(ConvertCommand),
    /// Validate a schema file
    Validate(ValidateCommand),
    /// Download StructureDefinitions from FHIR registry
    Download(DownloadCommand),
    /// Generate shell completion scripts
    Completion(CompletionCommand),
    /// Manage FHIRSchema repositories
    // Repository(RepositoryCommand),
    /// Generate code from FHIRSchema definitions
    Codegen(CodegenCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    human_panic::setup_panic!();

    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();

    info!("Starting FHIRSchema CLI");

    match cli.command {
        Commands::Convert(cmd) => cmd.execute(),
        Commands::Validate(cmd) => cmd.execute(),
        Commands::Download(cmd) => cmd.execute().await,
        Commands::Completion(cmd) => cmd.execute(),
        // Commands::Repository(cmd) => cmd.execute().await,
        Commands::Codegen(cmd) => cmd.execute().await,
    }
}
