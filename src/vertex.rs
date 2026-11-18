// SPDX-License-Identifier: MIT
/// Clip-space position and color shared with the native frame writer.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub position: [f32; 4],
    pub color: [f32; 4],
}
