// SPDX-License-Identifier: MIT
use std::ffi::c_void;

pub use crate::vertex::Vertex;
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Metrics {
    pub construct_ms: f64,
    pub submit_ms: f64,
    pub completion_ms: f64,
    pub present_ms: f64,
    pub draws: u32,
}
extern "C" {
    fn facet_open(width: *mut u32, height: *mut u32) -> *mut c_void;
    fn facet_frame(
        display: *mut c_void,
        vertices: *const Vertex,
        count: u32,
        metrics: *mut Metrics,
    ) -> i32;
    fn facet_close(display: *mut c_void) -> i32;
    fn facet_vertex_capacity() -> u32;
    fn facet_cpu_seconds() -> f64;
}
pub fn cpu_seconds() -> f64 {
    unsafe { facet_cpu_seconds() }
}
pub struct Display {
    raw: *mut c_void,
    pub width: u32,
    pub height: u32,
}
impl Display {
    pub fn open() -> Result<Self, String> {
        let (mut width, mut height) = (0, 0);
        let raw = unsafe { facet_open(&mut width, &mut height) };
        if raw.is_null() {
            Err("native display initialization failed".into())
        } else {
            Ok(Self { raw, width, height })
        }
    }
    pub fn capacity(&self) -> usize {
        unsafe { facet_vertex_capacity() as usize }
    }
    pub fn frame(&mut self, vertices: &[Vertex]) -> Result<(i32, Metrics), String> {
        if vertices.len() > self.capacity() {
            return Ok((2, Metrics::default()));
        }
        let mut metrics = Metrics::default();
        let result = unsafe {
            facet_frame(
                self.raw,
                vertices.as_ptr(),
                vertices.len() as u32,
                &mut metrics,
            )
        };
        if result < 0 {
            Err(format!("native frame failed ({})", result))
        } else {
            Ok((result, metrics))
        }
    }
    pub fn close(mut self) -> Result<(), String> {
        let result = unsafe { facet_close(self.raw) };
        self.raw = std::ptr::null_mut();
        if result == 0 {
            Ok(())
        } else {
            Err(format!("native cleanup failed ({})", result))
        }
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                facet_close(self.raw);
            }
        }
    }
}
