//! BetterRack domain + services. No HTTP framework here.

pub mod archive;
pub mod collate;
pub mod comic_data;
pub mod comic_info;
pub mod config;
pub mod db;
pub mod directories;
pub mod download;
pub mod downloads;
pub mod error;
pub mod jobs;
pub mod library;
pub mod pack;
pub mod pixeldrain;
pub mod rotating_fetch;
pub mod series;
pub mod settings;
pub mod store;
pub mod sync;
pub mod thumbnail;
pub mod uid;
pub mod wiki;

pub use error::{CoreError, Result};
