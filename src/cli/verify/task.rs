use std::{
    fmt::Display,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use colored::Colorize;
use log::{error, info};

use crate::checksum::{Checksum, ChecksumError};

#[derive(Debug)]
pub enum VerifyTaskStatus {
    Valid,
    Invalid,
    Missing,
}

impl VerifyTaskStatus {
    pub fn symbol(&self) -> &str {
        match self {
            VerifyTaskStatus::Valid => "✓",
            VerifyTaskStatus::Invalid => "✗",
            VerifyTaskStatus::Missing => "?",
        }
    }
}

impl Display for VerifyTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

#[derive(Debug)]
pub struct VerifyTaskResult {
    pub status: VerifyTaskStatus,
    pub filename: String,
    pub actual: Option<Checksum>,
    pub expected: Checksum,
}

impl Display for VerifyTaskResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            VerifyTaskStatus::Valid => write!(
                f,
                "{} {}",
                format!("{} {}", self.status, self.filename).green(),
                format!("({})", self.expected).dimmed()
            ),
            VerifyTaskStatus::Invalid => {
                write!(
                    f,
                    "{} {}",
                    format!("{} {}", self.status, self.filename).bold().red(),
                    format!(
                        "({} != {})",
                        format!("{}", self.actual.as_ref().unwrap()).red(),
                        format!("{}", self.expected)
                    )
                    .dimmed()
                )
            }
            VerifyTaskStatus::Missing => write!(
                f,
                "{}",
                format!("{} {}", self.status, self.filename).yellow()
            ),
        }
    }
}

#[derive(Debug)]
pub struct VerifyTaskError {
    pub filepath: String,
    pub message: String,
    pub error: Option<ChecksumError>,
}

impl Display for VerifyTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}: {}{}",
            self.filepath.dimmed(),
            self.message.red(),
            if let Some(error) = &self.error {
                format!(" ({})", error).red()
            } else {
                "".into()
            }
        )
    }
}

pub struct VerifyTaskCounters {
    pub valid: Arc<AtomicUsize>,
    pub invalid: Arc<AtomicUsize>,
    pub missing: Arc<AtomicUsize>,
}

pub struct VerifyTaskBuilder {
    worker_sempahore: Arc<tokio::sync::Semaphore>,
    chunk_size: usize,
    pub counters: Arc<VerifyTaskCounters>,
}

impl VerifyTaskBuilder {
    pub fn new(max_workers: usize, chunk_size: usize) -> Self {
        let counters = Arc::new(VerifyTaskCounters {
            valid: Arc::new(AtomicUsize::new(0)),
            invalid: Arc::new(AtomicUsize::new(0)),
            missing: Arc::new(AtomicUsize::new(0)),
        });

        Self {
            worker_sempahore: Arc::new(tokio::sync::Semaphore::new(max_workers)),
            chunk_size,
            counters,
        }
    }

    pub fn verify_checksum(
        &self,
        base_dirpath: PathBuf,
        filename: &str,
        expected: &Checksum,
    ) -> tokio::task::JoinHandle<Result<VerifyTaskResult, VerifyTaskError>> {
        let worker_permit = self.worker_sempahore.clone();
        let chunk_size = self.chunk_size;
        let counters = self.counters.clone();
        let filepath = base_dirpath.join(filename);

        let filename = String::from(filename);
        let expected = expected.clone();
        tokio::spawn(async move {
            let _permit = worker_permit
                .acquire()
                .await
                .expect("Failed to acquire worker permit");

            if !filepath.is_file() {
                counters.missing.fetch_add(1, Ordering::Relaxed);
                return Ok(VerifyTaskResult {
                    status: VerifyTaskStatus::Missing,
                    filename,
                    actual: None,
                    expected,
                });
            }

            let actual = Checksum::from_file(crate::checksum::ChecksumOptions {
                filepath: filepath.clone(),
                algorithm: expected.algorithm.clone(),
                mode: expected.mode,
                chunk_size: Some(chunk_size),
                progress_callback: None,
            })
            .await;

            match actual {
                Ok(actual) => {
                    let status = if actual == expected {
                        counters.valid.fetch_add(1, Ordering::Relaxed);
                        VerifyTaskStatus::Valid
                    } else {
                        counters.invalid.fetch_add(1, Ordering::Relaxed);
                        VerifyTaskStatus::Invalid
                    };

                    let result = VerifyTaskResult {
                        status,
                        filename,
                        actual: Some(actual),
                        expected,
                    };

                    info!("{:?}", result);
                    Ok(result)
                }
                Err(err) => {
                    let verify_error = VerifyTaskError {
                        filepath: filename,
                        message: String::from("Failed to calculate checksum"),
                        error: Some(err),
                    };

                    error!("{:?}", verify_error);
                    Err(verify_error)
                }
            }
        })
    }
}
