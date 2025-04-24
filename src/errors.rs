use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Can't receive packet")]
    NoPacket,
    #[error("Can't allocate memory")]
    NoMemory,
    #[error("{0}")]
    Netmap(#[from] netmap_rs::errors::Error),
    #[error("Too big packet: {0}")]
    TooBigPacket(usize),
    #[error("{0}")]
    Generic(#[from] io::Error),
    //#[error("{0}")]
    //Temporary(#[from] anyhow::Error),
    #[error("unknown error")]
    Unknown,
}
