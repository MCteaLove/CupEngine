use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// Геометрия на CPU. Загружается в GPU через `Renderer::create_mesh`.
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    fn new() -> Self {
        Self { vertices: Vec::new(), indices: Vec::new() }
    }

    /// Квад с центром `c`; `u` и `v` — полу-оси, u × v должно давать `n`.
    fn push_quad(&mut self, c: Vec3, n: Vec3, u: Vec3, v: Vec3) {
        let base = self.vertices.len() as u32;
        for p in [c - u - v, c + u - v, c + u + v, c - u + v] {
            self.vertices.push(Vertex { position: p.to_array(), normal: n.to_array() });
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Куб со стороной 1, центр в нуле.
    pub fn cube() -> Self {
        let mut m = Self::new();
        let faces = [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y, Vec3::X),
        ];
        for (n, u, v) in faces {
            m.push_quad(n * 0.5, n, u * 0.5, v * 0.5);
        }
        m
    }

    /// Плоскость в XZ, смотрит вверх, полу-размер 1 (масштабируйте трансформом).
    pub fn plane() -> Self {
        let mut m = Self::new();
        m.push_quad(Vec3::ZERO, Vec3::Y, Vec3::Z, Vec3::X);
        m
    }

    /// UV-сфера радиуса 0.5.
    pub fn sphere(stacks: u32, slices: u32) -> Self {
        let mut m = Self::new();
        for i in 0..=stacks {
            let theta = std::f32::consts::PI * i as f32 / stacks as f32;
            for j in 0..=slices {
                let phi = std::f32::consts::TAU * j as f32 / slices as f32;
                let n = Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
                m.vertices.push(Vertex { position: (n * 0.5).to_array(), normal: n.to_array() });
            }
        }
        for i in 0..stacks {
            for j in 0..slices {
                let a = i * (slices + 1) + j;
                let b = a + slices + 1;
                m.indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
            }
        }
        m
    }
}
