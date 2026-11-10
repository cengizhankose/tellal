//! Read-only reader for Chromium's `Local Storage/leveldb` (the Electron app's `localStorage`).
//!
//! Every `.ldb` table and `.log` write-ahead log in the directory is read into memory with one
//! `read` per file (a private snapshot: the live DB is never opened, locked or written), and for
//! each key the record with the highest sequence number wins (a deletion hides the key). Blocks
//! may be snappy-compressed; a minimal snappy decoder is included. Chromium stores keys as
//! `_<origin>\0<enc><key>` and values as `<enc><bytes>` where `<enc>` 1 = Latin-1, 0 = UTF-16LE.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, bail};

/// Internal record: (sequence, value or `None` for a deletion).
type Rec = (u64, Option<Vec<u8>>);

/// All live `localStorage` items of `origin` (e.g. `t3code://app`), decoded to strings.
pub fn local_storage(dir: &Path, origin: &str) -> anyhow::Result<HashMap<String, String>> {
    // Compaction can delete a file between listing and reading; retry with a fresh listing.
    let mut last_err = None;
    for _ in 0..3 {
        match read_all(dir) {
            Ok(recs) => {
                let prefix = format!("_{origin}\0");
                return Ok(recs
                    .into_iter()
                    .filter_map(|(k, (_, v))| {
                        let key = decode(k.strip_prefix(prefix.as_bytes())?)?;
                        Some((key, decode(&v?)?))
                    })
                    .collect());
            }
            Err(e) if e.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {
                last_err = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.expect("loop ran"))
}

fn read_all(dir: &Path) -> anyhow::Result<HashMap<Vec<u8>, Rec>> {
    let mut best: HashMap<Vec<u8>, Rec> = HashMap::new();
    let mut put = |key: &[u8], seq: u64, val: Option<&[u8]>| {
        if best.get(key).is_none_or(|(s, _)| *s < seq) {
            best.insert(key.to_vec(), (seq, val.map(<[u8]>::to_vec)));
        }
    };
    let mut files = 0;
    for entry in std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let path = entry?.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
        if ext != "ldb" && ext != "log" {
            continue;
        }
        let data = std::fs::read(&path)?;
        files += 1;
        let res = if ext == "ldb" { table(&data, &mut put) } else { log(&data, &mut put) };
        res.with_context(|| format!("parse {}", path.display()))?;
    }
    if files == 0 {
        bail!("no .ldb/.log files in {}", dir.display());
    }
    Ok(best)
}

fn varint(b: &[u8], i: &mut usize) -> anyhow::Result<u64> {
    let (mut v, mut shift) = (0u64, 0);
    loop {
        let c = *b.get(*i).context("truncated varint")?;
        *i += 1;
        v |= u64::from(c & 0x7f) << shift;
        if c < 0x80 {
            return Ok(v);
        }
        shift += 7;
        if shift > 63 {
            bail!("varint overflow");
        }
    }
}

fn slice(b: &[u8], start: usize, len: u64) -> anyhow::Result<&[u8]> {
    let end = start.checked_add(usize::try_from(len)?).context("length overflow")?;
    b.get(start..end).context("record out of bounds")
}

/// SSTable: footer → index block → data blocks.
fn table(d: &[u8], put: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) -> anyhow::Result<()> {
    if d.len() < 48 {
        bail!("table too short");
    }
    let footer = &d[d.len() - 48..];
    let mut i = 0;
    varint(footer, &mut i)?; // metaindex offset
    varint(footer, &mut i)?; // metaindex size
    let (io, is) = (varint(footer, &mut i)?, varint(footer, &mut i)?);
    let index = block(d, io, is)?;
    entries(&index, |_, handle| {
        let mut j = 0;
        let (bo, bs) = (varint(handle, &mut j)?, varint(handle, &mut j)?);
        let data = block(d, bo, bs)?;
        entries(&data, |ikey, val| {
            let Some(split) = ikey.len().checked_sub(8) else { bail!("short internal key") };
            let tag = u64::from_le_bytes(ikey[split..].try_into()?);
            put(&ikey[..split], tag >> 8, (tag & 0xff == 1).then_some(val));
            Ok(())
        })
    })
}

/// A block plus its 1-byte compression type (0 none, 1 snappy); the CRC is not checked.
fn block(d: &[u8], off: u64, size: u64) -> anyhow::Result<Vec<u8>> {
    let off = usize::try_from(off)?;
    let raw = slice(d, off, size)?;
    match d.get(off + raw.len()) {
        Some(0) => Ok(raw.to_vec()),
        Some(1) => snappy(raw),
        other => bail!("unknown block compression {other:?}"),
    }
}

/// Prefix-compressed entries of a block (restart array at the end).
fn entries(b: &[u8], mut f: impl FnMut(&[u8], &[u8]) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let n = b.len().checked_sub(4).context("short block")?;
    let restarts = u32::from_le_bytes(b[n..].try_into()?) as usize;
    let end = n.checked_sub(restarts * 4).context("bad restart count")?;
    let (mut i, mut key) = (0, Vec::new());
    while i < end {
        let shared = usize::try_from(varint(b, &mut i)?)?;
        let unshared = varint(b, &mut i)?;
        let vlen = varint(b, &mut i)?;
        if shared > key.len() {
            bail!("bad shared key length");
        }
        key.truncate(shared);
        key.extend_from_slice(slice(b, i, unshared)?);
        i += unshared as usize;
        let val = slice(b, i, vlen)?;
        i += vlen as usize;
        f(&key, val)?;
    }
    Ok(())
}

/// Write-ahead log: 32 KiB blocks of fragments → write batches.
fn log(d: &[u8], put: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) -> anyhow::Result<()> {
    const BLOCK: usize = 32 * 1024;
    let (mut i, mut rec) = (0, Vec::new());
    while i + 7 <= d.len() {
        let left = BLOCK - i % BLOCK;
        if left < 7 {
            i += left;
            continue;
        }
        let len = usize::from(u16::from_le_bytes([d[i + 4], d[i + 5]]));
        let kind = d[i + 6];
        // A record cut short by a concurrent writer ends the readable log.
        let Some(frag) = d.get(i + 7..i + 7 + len) else { break };
        i += 7 + len;
        match kind {
            0 => i += (BLOCK - i % BLOCK) % BLOCK, // zero padding (preallocated tail)
            1 => batch(frag, put)?,
            2 => rec = frag.to_vec(),
            3 => rec.extend_from_slice(frag),
            4 => {
                rec.extend_from_slice(frag);
                batch(&rec, put)?;
            }
            _ => break,
        }
    }
    Ok(())
}

fn batch(b: &[u8], put: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) -> anyhow::Result<()> {
    if b.len() < 12 {
        bail!("short batch");
    }
    let seq = u64::from_le_bytes(b[..8].try_into()?);
    let count = u32::from_le_bytes(b[8..12].try_into()?);
    let mut i = 12;
    for k in 0..u64::from(count) {
        let kind = *b.get(i).context("truncated batch")?;
        i += 1;
        let klen = varint(b, &mut i)?;
        let key = slice(b, i, klen)?;
        i += klen as usize;
        let val = if kind == 1 {
            let vlen = varint(b, &mut i)?;
            let v = slice(b, i, vlen)?;
            i += vlen as usize;
            Some(v)
        } else {
            None
        };
        put(key, seq + k, val);
    }
    Ok(())
}

/// Raw (unframed) snappy.
fn snappy(src: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut i = 0;
    let n = usize::try_from(varint(src, &mut i)?)?;
    let mut out = Vec::with_capacity(n);
    while i < src.len() {
        let tag = src[i];
        i += 1;
        let (len, off) = match tag & 3 {
            0 => {
                let mut len = usize::from(tag >> 2);
                if len >= 60 {
                    let w = len - 59;
                    let bytes = src.get(i..i + w).context("truncated literal length")?;
                    len = bytes.iter().rev().fold(0usize, |a, &b| (a << 8) | usize::from(b));
                    i += w;
                }
                let lit = src.get(i..i + len + 1).context("truncated literal")?;
                out.extend_from_slice(lit);
                i += len + 1;
                continue;
            }
            1 => {
                let b = *src.get(i).context("truncated copy")?;
                i += 1;
                (4 + usize::from((tag >> 2) & 7), (usize::from(tag >> 5) << 8) | usize::from(b))
            }
            2 => {
                let b = src.get(i..i + 2).context("truncated copy")?;
                i += 2;
                (usize::from(tag >> 2) + 1, usize::from(u16::from_le_bytes([b[0], b[1]])))
            }
            _ => {
                let b = src.get(i..i + 4).context("truncated copy")?;
                i += 4;
                (usize::from(tag >> 2) + 1, u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
            }
        };
        if off == 0 || off > out.len() {
            bail!("bad snappy offset");
        }
        let start = out.len() - off;
        for k in 0..len {
            out.push(out[start + k]);
        }
    }
    if out.len() != n {
        bail!("snappy length mismatch ({} != {n})", out.len());
    }
    Ok(out)
}

/// Chromium localStorage string: 1-byte encoding prefix, then Latin-1 (1) or UTF-16LE (0).
fn decode(b: &[u8]) -> Option<String> {
    let (enc, rest) = b.split_first()?;
    match enc {
        1 => Some(rest.iter().map(|&c| char::from(c)).collect()),
        0 if rest.len() % 2 == 0 => {
            String::from_utf16(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snappy_literal_and_copies() {
        // "abcabcabcX": literal "abc", copy len 6 offset 3 (1-byte offset form), literal "X".
        let src = [10, 2 << 2, b'a', b'b', b'c', 0b000_010_01, 3, 0, b'X'];
        assert_eq!(snappy(&src).unwrap(), b"abcabcabcX");
    }

    #[test]
    fn decodes_both_encodings() {
        assert_eq!(decode(&[1, b's', 0xfc]).as_deref(), Some("sü"));
        assert_eq!(decode(&[0, 0x31, 0x01, b'a', 0]).as_deref(), Some("ıa"));
    }

    #[test]
    fn log_batch_newest_wins_and_deletes() {
        let mut d = Vec::new();
        let mut rec = |seq: u64, ops: &[(u8, &[u8], &[u8])]| {
            let mut b = seq.to_le_bytes().to_vec();
            b.extend_from_slice(&(ops.len() as u32).to_le_bytes());
            for (kind, k, v) in ops {
                b.push(*kind);
                b.push(k.len() as u8);
                b.extend_from_slice(k);
                if *kind == 1 {
                    b.push(v.len() as u8);
                    b.extend_from_slice(v);
                }
            }
            d.extend_from_slice(&[0, 0, 0, 0]);
            d.extend_from_slice(&(b.len() as u16).to_le_bytes());
            d.push(1);
            d.extend_from_slice(&b);
        };
        rec(1, &[(1, b"a", b"old"), (1, b"b", b"x")]);
        rec(3, &[(1, b"a", b"new"), (0, b"b", b"")]);
        let mut best: HashMap<Vec<u8>, Rec> = HashMap::new();
        log(&d, &mut |k: &[u8], s, v: Option<&[u8]>| {
            if best.get(k).is_none_or(|(o, _)| *o < s) {
                best.insert(k.to_vec(), (s, v.map(<[u8]>::to_vec)));
            }
        })
        .unwrap();
        assert_eq!(best[b"a".as_slice()].1.as_deref(), Some(b"new".as_slice()));
        assert_eq!(best[b"b".as_slice()].1, None);
    }
}
