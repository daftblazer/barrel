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

    /// A node kind's name.
    pub fn name(&self, kind: u32) -> &str {
        self.schema.nodes.get(&kind).map_or("?", |n| n.0.as_str())
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
