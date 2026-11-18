// SPDX-License-Identifier: MIT
//! Shared geometry and optional Linux console presentation for Poulygon applications.
pub mod engine;
pub mod geometry;
pub mod vertex;
#[cfg(feature = "native")]
pub mod platform;
