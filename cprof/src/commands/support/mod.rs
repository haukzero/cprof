//! Shared command workflows. [`write`] owns scope selection and lock lifetimes;
//! profile, package, and editor code implement operations without acquiring locks.
//! Retryable calls must finish their write session before elevation begins.

pub(crate) mod adopt;
pub(crate) mod clean;
pub(crate) mod config;
pub(crate) mod create;
pub(crate) mod edit;
pub(crate) mod pack;
pub(crate) mod remove;
pub(crate) mod rename;
pub(crate) mod selection;
pub(crate) mod unpack;
pub(crate) mod write;
