use std::io;

use binder::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DumpError {
    #[error("IO Error")]
    IO(#[from] io::Error),
    #[error("Dump error")]
    DumpStatus(#[from] StatusCode),
}
