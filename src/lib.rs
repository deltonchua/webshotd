//! Web screenshot server.
//!
//! Terminology:
//! - `vw`/`vh`: viewport width and height (px)
//! - `frames`: number of vertical chunks a page is sliced into
//! - `format`: screenshot output format ({jpeg, png, webp})

pub mod browser;
pub mod cli;
pub mod error;
pub mod image;
pub mod logging;
pub mod routes;
