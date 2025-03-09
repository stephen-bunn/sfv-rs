mod generate;
mod verify;

use std::{env::current_dir, path::PathBuf, thread};

use clap::Parser;

use crate::{
    checksum::{ChecksumAlgorithm, ChecksumMode, DEFAULT_CHUNK_SIZE},
    manifest::ManifestFormat,
};

#[derive(Debug, clap::Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Option<Commands>,
    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 0)]
    /// Verbosity level
    pub verbosity: u8,
    /// Disable color output
    #[arg(long, default_value_t = false)]
    pub no_color: bool,
    /// Disable progress output
    #[arg(long, default_value_t = false)]
    pub no_progress: bool,
}

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Generate a new manifest
    Generate {
        /// Pattern to match files against
        #[arg(value_parser = clap::value_parser!(PathBuf))]
        dirpath: PathBuf,
        #[arg(short, long, default_value = None)]
        /// Path to output the manifest file to
        output: Option<PathBuf>,
        #[arg(short, long, default_value = None)]
        /// Algorithm to use for checksum calculation
        algorithm: Option<ChecksumAlgorithm>,
        /// Format of the manifest file
        #[arg(short, long, default_value = "sfv")]
        format: Option<ManifestFormat>,
        #[arg(short, long, default_value = "binary")]
        /// Checksum mode to use for generating checksums
        mode: Option<ChecksumMode>,
        /// Chunk size to use for generating checksums
        #[arg(short, long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,
        /// Maximum number of workers to use
        #[arg(short = 'x', long = "max-workers", default_value = "8")]
        max_workers: usize,
        /// Verbosity level
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbosity: Option<u8>,
    },

    /// Verify a manifest file in the given directory
    Verify {
        /// Path to the directory containing the files to verify
        #[arg(value_parser = clap::value_parser!(PathBuf))]
        dirpath: PathBuf,
        /// Path to the manifest file to verify
        #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
        manifest: Option<PathBuf>,
        /// Chunk size to use for generating checksums
        #[arg(short, long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,
        /// Maximum number of workers to use
        #[arg(short = 'x', long = "max-workers", default_value = "8")]
        max_workers: usize,
        /// Verbosity level
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbosity: Option<u8>,
    },
}

pub async fn cli() -> anyhow::Result<()> {
    let args = Cli::parse();
    if args.no_color {
        colored::control::set_override(false);
    }

    match args.command {
        Some(Commands::Generate {
            dirpath,
            output,
            algorithm,
            format,
            mode,
            chunk_size,
            max_workers,
            verbosity,
        }) => {
            generate::generate(generate::GenerateOptions {
                dirpath,
                output,
                algorithm,
                format,
                mode,
                chunk_size,
                max_workers,
                show_progress: !args.no_progress,
                verbosity: verbosity.unwrap_or(args.verbosity),
            })
            .await?;
        }
        Some(Commands::Verify {
            dirpath,
            manifest,
            chunk_size,
            max_workers,
            verbosity,
        }) => {
            verify::verify(verify::VerifyOptions {
                dirpath,
                manifest,
                chunk_size,
                max_workers,
                show_progress: !args.no_progress,
                verbosity: verbosity.unwrap_or(args.verbosity),
            })
            .await?;
        }
        None => {
            verify::verify(verify::VerifyOptions {
                dirpath: current_dir().unwrap(),
                manifest: None,
                chunk_size: DEFAULT_CHUNK_SIZE,
                max_workers: thread::available_parallelism()?.get(),
                show_progress: !args.no_progress,
                verbosity: args.verbosity,
            })
            .await?
        }
    }

    Ok(())
}
