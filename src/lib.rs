// SPDX-License-Identifier: MIT
//! Shared geometry and optional Linux console presentation for Poulygon applications.
pub mod engine;
pub mod geometry;
#[cfg(feature = "native")]
pub mod input;
#[cfg(feature = "native")]
pub mod platform;
pub mod vertex;
