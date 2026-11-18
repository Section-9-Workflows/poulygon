// SPDX-License-Identifier: MIT
//! Retained colored quads, expanded only when preparing a visible frame.

pub use crate::vertex::Vertex;

/// Four corners in perimeter order; the diagonal joins corners zero and two.
#[derive(Clone, Copy)]
pub struct Quad {
    points: [[f32; 3]; 4],
    colors: [[u8; 4]; 4],
}

impl Quad {
    pub fn new(points: [[f32; 3]; 4], colors: [[u8; 4]; 4]) -> Result<Self, &'static str> {
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return Err("non-finite quad position");
        }
        Ok(Self { points, colors })
    }
}

/// Reuses frame storage and refuses overflow before submitting anything to the GPU.
pub struct Frame {
    vertices: Vec<Vertex>,
    capacity: usize,
}

impl Frame {
    pub fn new(capacity: usize) -> Self {
        Self { vertices: Vec::with_capacity(capacity), capacity }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Appends a retained mesh using a column-major local-to-clip matrix.
    /// A refused mesh leaves the previous frame contents intact.
    pub fn append(&mut self, quads: &[Quad], matrix: [[f32; 4]; 4]) -> Result<(), &'static str> {
        if matrix.iter().flatten().any(|v| !v.is_finite()) {
            return Err("non-finite view matrix");
        }
        let start = self.vertices.len();
        for quad in quads {
            let clip = quad.points.map(|p| std::array::from_fn::<_, 4, _>(|row| {
                matrix[0][row] * p[0] + matrix[1][row] * p[1]
                    + matrix[2][row] * p[2] + matrix[3][row]
            }));
            if clip.iter().flatten().any(|v| !v.is_finite()) {
                self.vertices.truncate(start);
                return Err("non-finite projected position");
            }
            // SGX clips intersecting faces; only wholly outside faces are rejected here.
            if (0..3).any(|axis| clip.iter().all(|p| p[axis] < -p[3])
                || clip.iter().all(|p| p[axis] > p[3])) {
                continue;
            }
            if self.capacity.saturating_sub(self.vertices.len()) < 6 {
                self.vertices.truncate(start);
                return Err("visible geometry exceeds the native frame capacity");
            }
            for corner in [0, 1, 2, 0, 2, 3] {
                self.vertices.push(Vertex {
                    position: clip[corner],
                    color: quad.colors[corner].map(|v| f32::from(v) / 255.0),
                });
            }
        }
        Ok(())
    }
}
