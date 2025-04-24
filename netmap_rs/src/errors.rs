use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    OpenError(&'static str),
    #[error("unknown data store error")]
    Unknown,
}
