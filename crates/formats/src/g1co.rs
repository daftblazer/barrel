//! A field's collision (`<AREA>_col.g1co` in its container): `OC1G`, then one `HVB_` chunk, a
//! bounding-volume hierarchy over a triangle mesh (`docs/formats.md`, "Collision"). Little-endian.

use crate::Error;

/// A node of the hierarchy: its bounds, its kind (0 a leaf, 2 to 4 an inner node: presumably
/// the axis it splits), the link to its second child (65535 in a leaf), and the triangles it
/// covers (`first..first + count`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub max: [f32; 3],
    pub min: [f32; 3],
    pub kind: i32,
    pub link: i32,
    pub count: u32,
    pub first: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Collision {
    pub nodes: Vec<Node>,
    /// Each triangle's three vertex numbers.
    pub triangles: Vec<[u16; 3]>,
    pub vertices: Vec<[f32; 3]>,
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(d.get(at..at + 4).ok_or(Error::Corrupt("g1co: truncated"))?.try_into().unwrap()))
}

fn f32_at(d: &[u8], at: usize) -> Result<f32, Error> {
    u32_at(d, at).map(f32::from_bits)
}

pub fn parse(d: &[u8]) -> Result<Collision, Error> {
    if d.get(..4) != Some(b"OC1G") {
        return Err(Error::NotThisFormat);
    }
    // "OC1G", the version (50, "2"), 0x01010000, then the size written (a container's entry
    // can run past it).
    let len = u32_at(d, 0xc)? as usize;
    if len > d.len() {
        return Err(Error::Corrupt("g1co: size past the data"));
    }
    let d = &d[..len];
    // The chunk: "HVB_", its version, 0, its size, then its counts and offsets (from the
    // chunk's start).
    let c = 0x10;
    if d.get(c..c + 4) != Some(b"HVB_") {
        return Err(Error::Corrupt("g1co: no HVB_ chunk"));
    }
    let size = u32_at(d, c + 0xc)? as usize;
    let (node_count, node_size) = (u32_at(d, c + 0x10)? as usize, u32_at(d, c + 0x14)? as usize);
    let (triangle_count, triangles_at) = (u32_at(d, c + 0x18)? as usize, u32_at(d, c + 0x1c)? as usize);
    let (vertex_count, vertices_at) = (u32_at(d, c + 0x20)? as usize, u32_at(d, c + 0x24)? as usize);
    if node_size != 48 || c + size > d.len() || c + vertices_at + 16 * vertex_count != c + size {
        return Err(Error::Corrupt("g1co: chunk layout not known"));
    }
    let nodes = (0..node_count)
        .map(|k| {
            let a = c + 0x30 + 48 * k;
            let v = |at: usize| -> Result<[f32; 3], Error> { Ok([f32_at(d, at)?, f32_at(d, at + 4)?, f32_at(d, at + 8)?]) };
            Ok(Node {
                max: v(a)?,
                min: v(a + 16)?,
                kind: u32_at(d, a + 32)? as i32,
                link: u32_at(d, a + 36)? as i32,
                count: u32_at(d, a + 40)?,
                first: u32_at(d, a + 44)?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let triangles = (0..triangle_count)
        .map(|k| {
            let a = c + triangles_at + 6 * k;
            let b = d.get(a..a + 6).ok_or(Error::Corrupt("g1co: truncated"))?;
            Ok([u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]), u16::from_le_bytes([b[4], b[5]])])
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let vertices = (0..vertex_count)
        .map(|k| {
            let a = c + vertices_at + 16 * k;
            Ok([f32_at(d, a)?, f32_at(d, a + 4)?, f32_at(d, a + 8)?])
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Collision { nodes, triangles, vertices })
}

impl Collision {
    /// What is wrong with the hierarchy, if anything: an index past the vertices, leaves that
    /// do not cover every triangle once, or a leaf's triangle outside its bounds.
    pub fn check(&self) -> Option<String> {
        if let Some(t) = self.triangles.iter().find(|t| t.iter().any(|&i| i as usize >= self.vertices.len())) {
            return Some(format!("triangle {t:?} past the {} vertices", self.vertices.len()));
        }
        let mut covered = vec![0u32; self.triangles.len()];
        for n in self.nodes.iter().filter(|n| n.kind == 0) {
            for t in n.first..n.first + n.count {
                let Some(c) = covered.get_mut(t as usize) else { return Some(format!("leaf past the triangles: {n:?}")) };
                *c += 1;
                let inside = self.triangles[t as usize].iter().all(|&i| {
                    let v = self.vertices[i as usize];
                    (0..3).all(|a| v[a] >= n.min[a] - 1.0 && v[a] <= n.max[a] + 1.0)
                });
                if !inside {
                    return Some(format!("triangle {t} outside its leaf {n:?}"));
                }
            }
        }
        if covered.iter().any(|&c| c != 1) {
            return Some(format!("{} triangles not in exactly one leaf", covered.iter().filter(|&&c| c != 1).count()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_hierarchy_reads() {
        // One leaf over one triangle of three vertices.
        let mut chunk = Vec::new();
        chunk.extend(b"HVB_");
        chunk.extend(50u32.to_le_bytes());
        chunk.extend(0u32.to_le_bytes());
        let (nodes_at, tris_at) = (0x30usize, 0x30 + 48);
        let verts_at = tris_at + 8;
        let size = verts_at + 3 * 16;
        chunk.extend((size as u32).to_le_bytes());
        for v in [1u32, 48, 1, tris_at as u32, 3, verts_at as u32] {
            chunk.extend(v.to_le_bytes());
        }
        chunk.resize(nodes_at, 0);
        for f in [1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
            chunk.extend(f.to_le_bytes());
        }
        for v in [0i32, 65535, 1, 0] {
            chunk.extend(v.to_le_bytes());
        }
        for i in [0u16, 1, 2, 0] {
            chunk.extend(i.to_le_bytes());
        }
        for v in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]] {
            for f in v.into_iter().chain([0.0]) {
                chunk.extend(f.to_le_bytes());
            }
        }
        let mut file = b"OC1G".to_vec();
        file.extend(50u32.to_le_bytes());
        file.extend(0x0101_0000u32.to_le_bytes());
        file.extend((16 + chunk.len() as u32).to_le_bytes());
        file.extend(chunk);
        let c = parse(&file).unwrap();
        assert_eq!((c.nodes.len(), c.triangles.clone(), c.vertices.len()), (1, vec![[0, 1, 2]], 3));
        assert_eq!(c.check(), None);
    }
}
