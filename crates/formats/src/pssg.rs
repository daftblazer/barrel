//! PSSG files: Ayesha DX's models, maps and animations (`Res/x64/**/*.PSSG.gz`, plain PSSG
//! despite the name). Big-endian. A schema names the node kinds and their attribute kinds, then
//! a tree of nodes follows, each with attributes and either child nodes or raw data
//! (`docs/formats.md`, "PSSG").

use crate::Error;

/// A node kind of the schema: its name and its attribute kinds' names, by id.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub nodes: std::collections::HashMap<u32, (String, std::collections::HashMap<u32, String>)>,
}

#[derive(Clone, Debug)]
pub struct Node<'a> {
    pub kind: u32,
    /// Each attribute's kind and its bytes.
    pub attributes: Vec<(u32, &'a [u8])>,
    pub children: Vec<Node<'a>>,
    /// The bytes after the attributes when they are not child nodes.
    pub data: &'a [u8],
}

/// A file's PSSG bytes: as they are, or ungzipped (most motions are gzipped, `1f 8b`; the
/// rest of the `.PSSG.gz` files are plain).
pub fn unwrap(d: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    if d.starts_with(&[0x1f, 0x8b]) {
        Ok(std::borrow::Cow::Owned(crate::inflate::gunzip(d, 512 << 20)?))
    } else {
        Ok(std::borrow::Cow::Borrowed(d))
    }
}

pub struct Pssg<'a> {
    pub schema: Schema,
    pub root: Node<'a>,
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, Error> {
    Ok(u32::from_be_bytes(d.get(at..at + 4).ok_or(Error::Corrupt("pssg: truncated"))?.try_into().unwrap()))
}

fn name_at(d: &[u8], at: usize) -> Result<(String, usize), Error> {
    let len = u32_at(d, at)? as usize;
    let bytes = d.get(at + 4..at + 4 + len).ok_or(Error::Corrupt("pssg: truncated"))?;
    Ok((String::from_utf8_lossy(bytes).into_owned(), at + 4 + len))
}

impl<'a> Pssg<'a> {
    pub fn parse(d: &'a [u8]) -> Result<Pssg<'a>, Error> {
        if d.get(..4) != Some(b"PSSG") {
            return Err(Error::NotThisFormat);
        }
        let size = u32_at(d, 4)? as usize;
        if size + 8 != d.len() {
            return Err(Error::Corrupt("pssg: size does not match the file"));
        }
        let node_kinds = u32_at(d, 12)?;
        let mut at = 16;
        let mut schema = Schema::default();
        for _ in 0..node_kinds {
            let id = u32_at(d, at)?;
            let (name, next) = name_at(d, at + 4)?;
            let count = u32_at(d, next)?;
            at = next + 4;
            let mut attrs = std::collections::HashMap::new();
            for _ in 0..count {
                let aid = u32_at(d, at)?;
                let (aname, next) = name_at(d, at + 4)?;
                attrs.insert(aid, aname);
                at = next;
            }
            schema.nodes.insert(id, (name, attrs));
        }
        let (root, end) = node(d, at, &schema, 0)?;
        if end != d.len() {
            return Err(Error::Corrupt("pssg: bytes after the root node"));
        }
        Ok(Pssg { schema, root })
    }

    /// An attribute kind's name, looked up across the schema (attribute ids are the file's, not
    /// a node kind's: a node carries kinds listed under others, its `id` among them).
    pub fn attribute_name(&self, id: u32) -> &str {
        self.schema.nodes.values().find_map(|(_, a)| a.get(&id)).map_or("?", |s| s.as_str())
    }

    /// A node kind's name.
    pub fn name(&self, kind: u32) -> &str {
        self.schema.nodes.get(&kind).map_or("?", |n| n.0.as_str())
    }
}

/// A mesh read out of a `RENDERDATASOURCE`: its triangles' corners (three indices each) and
/// the vertex streams its `RENDERSTREAM`s name, by their `renderType` (`Vertex`, `Normal`,
/// `Color`, `ST` ...), each element as floats.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub name: String,
    pub indices: Vec<u32>,
    pub streams: Vec<(String, Vec<Vec<f32>>)>,
}

impl Mesh {
    pub fn stream(&self, render_type: &str) -> Option<&[Vec<f32>]> {
        self.streams.iter().find(|(t, _)| t == render_type).map(|(_, v)| v.as_slice())
    }

    /// The vertices' positions: `Vertex`, or `SkinnableVertex` in a skinned mesh.
    pub fn positions(&self) -> &[Vec<f32>] {
        self.stream("Vertex").or_else(|| self.stream("SkinnableVertex")).unwrap_or(&[])
    }
}

impl Pssg<'_> {
    fn attr<'n>(&self, n: &'n Node, name: &str) -> Option<&'n [u8]> {
        n.attributes.iter().find(|(id, _)| self.attribute_name(*id) == name).map(|(_, b)| *b)
    }

    /// A string attribute (u32 length, bytes).
    pub fn attr_str<'n>(&self, n: &'n Node, name: &str) -> Option<&'n str> {
        let b = self.attr(n, name)?;
        let len = u32::from_be_bytes(b.get(..4)?.try_into().ok()?) as usize;
        std::str::from_utf8(b.get(4..4 + len)?).ok()
    }

    pub fn attr_u32(&self, n: &Node, name: &str) -> Option<u32> {
        Some(u32::from_be_bytes(self.attr(n, name)?.get(..4)?.try_into().ok()?))
    }

    fn walk<'n>(&self, n: &'n Node<'n>, out: &mut Vec<&'n Node<'n>>) {
        out.push(n);
        for c in &n.children {
            self.walk(c, out);
        }
    }

    /// Where each mesh is drawn: the data source an instance names (`RENDERSTREAMINSTANCE` or
    /// `MODIFIERNETWORKINSTANCE`, `indices="#<source>"`) with its node's world matrix, the scene
    /// nodes' `TRANSFORM`s (16 big-endian floats, row vectors: the translation in the last row)
    /// multiplied from the root down.
    pub fn placements(&self) -> Vec<(String, [f32; 16])> {
        const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
            let mut m = [0.0; 16];
            for r in 0..4 {
                for c in 0..4 {
                    m[r * 4 + c] = (0..4).map(|k| a[r * 4 + k] * b[k * 4 + c]).sum();
                }
            }
            m
        }
        let mut out = Vec::new();
        let mut stack: Vec<(&Node, [f32; 16])> = vec![(&self.root, IDENTITY)];
        while let Some((n, parent)) = stack.pop() {
            let kind = self.name(n.kind);
            let local = n
                .children
                .iter()
                .find(|c| self.name(c.kind) == "TRANSFORM" && c.data.len() == 64)
                .map(|t| std::array::from_fn(|k| f32::from_be_bytes(t.data[4 * k..4 * k + 4].try_into().unwrap())));
            let world = match (kind.ends_with("NODE"), local) {
                (true, Some(l)) => mul(&l, &parent),
                _ => parent,
            };
            if kind == "RENDERSTREAMINSTANCE" || kind == "MODIFIERNETWORKINSTANCE" {
                if let Some(src) = self.attr_str(n, "indices") {
                    out.push((src.trim_start_matches('#').to_string(), world));
                }
            }
            for c in &n.children {
                stack.push((c, world));
            }
        }
        out
    }

    /// Every mesh in the file (its vertex data big-endian like the rest: a unit normal and a
    /// white vertex colour read so, noise the other way round).
    pub fn meshes(&self) -> Vec<Mesh> {
        let mut all = Vec::new();
        self.walk(&self.root, &mut all);
        let kind = |n: &Node| self.name(n.kind).to_string();
        let blocks: std::collections::HashMap<&str, &Node> =
            all.iter().filter(|n| kind(n) == "DATABLOCK").filter_map(|n| Some((self.attr_str(n, "id")?, *n))).collect();
        let f32_at = |b: &[u8], at: usize| -> Option<f32> {
            let w: [u8; 4] = b.get(at..at + 4)?.try_into().ok()?;
            Some(f32::from_be_bytes(w))
        };
        let mut out = Vec::new();
        for src in all.iter().filter(|n| kind(n) == "RENDERDATASOURCE") {
            let mut mesh = Mesh { name: self.attr_str(src, "id").unwrap_or("").to_string(), ..Default::default() };
            for c in &src.children {
                match kind(c).as_str() {
                    "RENDERINDEXSOURCE" => {
                        let data = c.children.iter().find(|d| kind(d) == "INDEXSOURCEDATA").map_or(&[][..], |d| d.data);
                        let raw: Vec<u32> = match self.attr_str(c, "format") {
                            Some("uint") => data.as_chunks::<4>().0.iter().map(|w| u32::from_be_bytes(*w)).collect(),
                            Some("uchar") => data.iter().map(|&b| u32::from(b)).collect(),
                            _ => data.as_chunks::<2>().0.iter().map(|w| u32::from(u16::from_be_bytes(*w))).collect(),
                        };
                        mesh.indices = match self.attr_str(c, "primitive") {
                            Some("triangles") => raw,
                            // A strip: each corner after the first two makes a triangle, every
                            // other one turned, the degenerate ones (restarts) left out.
                            _ => raw
                                .windows(3)
                                .enumerate()
                                .filter(|(_, t)| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
                                .flat_map(|(k, t)| if k % 2 == 0 { [t[0], t[1], t[2]] } else { [t[1], t[0], t[2]] })
                                .collect(),
                        };
                    }
                    "RENDERSTREAM" => {
                        let Some(block) = self.attr_str(c, "dataBlock").and_then(|b| blocks.get(b.trim_start_matches('#'))) else { continue };
                        let sub = self.attr_u32(c, "subStream").unwrap_or(0) as usize;
                        let Some(st) = block.children.iter().filter(|d| kind(d) == "DATABLOCKSTREAM").nth(sub) else { continue };
                        let data = block.children.iter().find(|d| kind(d) == "DATABLOCKDATA").map_or(&[][..], |d| d.data);
                        let count = self.attr_u32(block, "elementCount").unwrap_or(0) as usize;
                        let (offset, stride) = (self.attr_u32(st, "offset").unwrap_or(0) as usize, self.attr_u32(st, "stride").unwrap_or(0) as usize);
                        let data_type = self.attr_str(st, "dataType").unwrap_or("");
                        let values = if data_type == "uchar4" {
                            // Bytes (the skin's joint numbers), as they are.
                            (0..count).map_while(|e| data.get(offset + e * stride..offset + e * stride + 4).map(|b| b.iter().map(|&v| f32::from(v)).collect())).collect()
                        } else {
                            let width = match data_type {
                                "float4" => 4,
                                "float3" => 3,
                                "float2" => 2,
                                "float1" | "float" => 1,
                                _ => continue,
                            };
                            (0..count).map_while(|e| (0..width).map(|k| f32_at(data, offset + e * stride + 4 * k)).collect::<Option<Vec<f32>>>()).collect()
                        };
                        mesh.streams.push((self.attr_str(st, "renderType").unwrap_or("?").to_string(), values));
                    }
                    _ => {}
                }
            }
            out.push(mesh);
        }
        out
    }
}

/// The node at `at` and the offset after it.
fn node<'a>(d: &'a [u8], at: usize, schema: &Schema, depth: usize) -> Result<(Node<'a>, usize), Error> {
    if depth > 64 {
        return Err(Error::Corrupt("pssg: nested too deeply"));
    }
    let kind = u32_at(d, at)?;
    if !schema.nodes.contains_key(&kind) {
        return Err(Error::Corrupt("pssg: node kind not in the schema"));
    }
    let size = u32_at(d, at + 4)? as usize;
    let end = at + 8 + size;
    if end > d.len() {
        return Err(Error::Corrupt("pssg: truncated"));
    }
    let attr_size = u32_at(d, at + 8)? as usize;
    let mut p = at + 12;
    let attr_end = p + attr_size;
    if attr_end > end {
        return Err(Error::Corrupt("pssg: attributes past the node"));
    }
    let mut attributes = Vec::new();
    while p < attr_end {
        let id = u32_at(d, p)?;
        let len = u32_at(d, p + 4)? as usize;
        let bytes = d.get(p + 8..p + 8 + len).ok_or(Error::Corrupt("pssg: truncated"))?;
        if p + 8 + len > attr_end {
            return Err(Error::Corrupt("pssg: attribute past its block"));
        }
        attributes.push((id, bytes));
        p += 8 + len;
    }
    // What follows is child nodes when they read to the node's end exactly, raw data otherwise.
    let rest = &d[attr_end..end];
    let children = children(d, attr_end, end, schema, depth).unwrap_or_default();
    let data = if children.is_empty() { rest } else { &rest[..0] };
    Ok((Node { kind, attributes, children, data }, end))
}

fn children<'a>(d: &'a [u8], mut at: usize, end: usize, schema: &Schema, depth: usize) -> Option<Vec<Node<'a>>> {
    let mut out = Vec::new();
    while at < end {
        let (n, next) = node(d, at, schema, depth + 1).ok()?;
        if next > end {
            return None;
        }
        out.push(n);
        at = next;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn be(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }

    #[test]
    fn a_small_tree_reads() {
        // Schema: node 1 "ROOT" with attribute 1 "id"; node 2 "DATA".
        let mut body = Vec::new();
        body.extend(be(1));
        body.extend(be(2));
        for (id, name, attrs) in [(1u32, "ROOT", vec![(1u32, "id")]), (2, "DATA", vec![])] {
            body.extend(be(id));
            body.extend(be(name.len() as u32));
            body.extend(name.as_bytes());
            body.extend(be(attrs.len() as u32));
            for (aid, an) in attrs {
                body.extend(be(aid));
                body.extend(be(an.len() as u32));
                body.extend(an.as_bytes());
            }
        }
        // DATA node: no attributes, 3 data bytes.
        let mut data = Vec::new();
        data.extend(be(2));
        data.extend(be(4 + 3));
        data.extend(be(0));
        data.extend([7, 8, 9]);
        // ROOT node: attribute id = "x", then the DATA child.
        let mut attrs = Vec::new();
        attrs.extend(be(1));
        attrs.extend(be(1));
        attrs.push(b'x');
        let mut root = Vec::new();
        root.extend(be(1));
        root.extend(be((4 + attrs.len() + data.len()) as u32));
        root.extend(be(attrs.len() as u32));
        root.extend(&attrs);
        root.extend(&data);
        body.extend(&root);
        let mut file = b"PSSG".to_vec();
        file.extend(be(body.len() as u32));
        file.extend(body);
        let p = Pssg::parse(&file).unwrap();
        assert_eq!(p.name(p.root.kind), "ROOT");
        assert_eq!(p.root.attributes, [(1, &b"x"[..])]);
        assert_eq!(p.root.children.len(), 1);
        assert_eq!((p.name(p.root.children[0].kind), p.root.children[0].data), ("DATA", &[7u8, 8, 9][..]));
    }
}
