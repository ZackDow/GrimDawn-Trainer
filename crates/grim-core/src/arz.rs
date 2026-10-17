//! Native ARZ v3 codec. No game executable or Windows toolchain is required.
//! Format provenance and verification are recorded in docs/FORMATS.md.
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Field {
    pub kind: u16,
    pub values: Vec<Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    Text(String),
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Record {
    pub name: String,
    pub class: String,
    pub time: u64,
    pub fields: BTreeMap<String, Field>,
}
impl Record {
    pub fn number(&self, key: &str) -> Option<f64> {
        match self.fields.get(key)?.values.first()? {
            Value::Number(v) => Some(*v),
            _ => None,
        }
    }
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.fields.get(key)?.values.first()? {
            Value::Text(v) => Some(v),
            _ => None,
        }
    }
}

pub(crate) struct Cursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).context("Offset overflow")?;
        let out = self
            .data
            .get(self.pos..end)
            .context("Truncated binary file")?;
        self.pos = end;
        Ok(out)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
    }
    pub fn string(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        ensure!(len < 16_000_000, "Unreasonable string size");
        Ok(self.take(len)?.iter().map(|&b| char::from(b)).collect())
    }
}
fn index(strings: &[String], i: u32) -> Result<String> {
    strings
        .get(i as usize)
        .cloned()
        .context("Invalid ARZ string reference")
}

pub fn read(bytes: &[u8], wanted: impl Fn(&str) -> bool) -> Result<BTreeMap<String, Record>> {
    read_selected(bytes, wanted, |_| true)
}
pub fn read_selected(
    bytes: &[u8],
    wanted: impl Fn(&str) -> bool,
    keep: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, Record>> {
    let mut h = Cursor::new(bytes);
    ensure!(
        h.u16()? == 2 && h.u16()? == 3,
        "Only Grim Dawn ARZ version 3 is supported"
    );
    let table_start = h.u32()? as usize;
    let table_size = h.u32()? as usize;
    let record_count = h.u32()? as usize;
    let strings_start = h.u32()? as usize;
    let strings_size = h.u32()? as usize;
    ensure!(
        table_start >= 24 && table_start.checked_add(table_size) == Some(strings_start),
        "Invalid ARZ table layout"
    );
    ensure!(
        record_count <= 1_000_000 && strings_size <= bytes.len(),
        "Invalid ARZ counts"
    );
    let strings_end = strings_start
        .checked_add(strings_size)
        .context("ARZ offset overflow")?;
    let mut s = Cursor::new(
        bytes
            .get(strings_start..strings_end)
            .context("Truncated ARZ strings")?,
    );
    let count = s.u32()? as usize;
    ensure!(count <= 4_000_000, "Too many ARZ strings");
    let strings = (0..count).map(|_| s.string()).collect::<Result<Vec<_>>>()?;
    ensure!(s.pos == s.data.len(), "Unexpected ARZ string data");
    let mut t = Cursor::new(
        bytes
            .get(table_start..strings_start)
            .context("Truncated ARZ table")?,
    );
    let mut out = BTreeMap::new();
    for _ in 0..record_count {
        let name = index(&strings, t.u32()?)?.replace('\\', "/").to_lowercase();
        let class = t.string()?;
        let offset = (t.u32()? as usize)
            .checked_add(24)
            .context("ARZ offset overflow")?;
        let packed = t.u32()? as usize;
        let unpacked = t.u32()? as usize;
        let time = t.u64()?;
        ensure!(
            offset
                .checked_add(packed)
                .is_some_and(|end| end <= table_start),
            "Record outside ARZ data region"
        );
        ensure!(unpacked <= 32_000_000, "Record too large");
        if !wanted(&name) {
            continue;
        }
        let raw = lz4_flex::block::decompress(&bytes[offset..offset + packed], unpacked)
            .context("Invalid ARZ compression")?;
        let mut r = Cursor::new(&raw);
        let mut fields = BTreeMap::new();
        while r.pos < raw.len() {
            let kind = r.u16()?;
            ensure!(kind <= 3, "Unknown ARZ field type {kind}");
            let count = r.u16()? as usize;
            let key = index(&strings, r.u32()?)?;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                let bits = r.u32()?;
                values.push(match kind {
                    2 => Value::Text(index(&strings, bits)?),
                    1 => {
                        let v = f32::from_bits(bits);
                        ensure!(v.is_finite(), "Nonfinite database value");
                        Value::Number(v as f64)
                    }
                    _ => Value::Number((bits as i32) as f64),
                });
            }
            if keep(&key) {
                ensure!(
                    fields.insert(key.clone(), Field { kind, values }).is_none(),
                    "Duplicate field {key} in {name}"
                );
            }
        }
        ensure!(
            out.insert(
                name.clone(),
                Record {
                    name,
                    class,
                    time,
                    fields
                }
            )
            .is_none(),
            "Duplicate ARZ record"
        );
    }
    ensure!(t.pos == t.data.len(), "Unexpected ARZ record-table data");
    Ok(out)
}

fn word(out: &mut Vec<u8>, n: usize) -> Result<()> {
    out.extend_from_slice(&u32::try_from(n)?.to_le_bytes());
    Ok(())
}
fn latin(text: &str) -> Result<Vec<u8>> {
    text.chars()
        .map(|c| u8::try_from(c as u32).context("ARZ strings must be Latin-1"))
        .collect()
}
fn string(out: &mut Vec<u8>, s: &str) -> Result<()> {
    let bytes = latin(s)?;
    word(out, bytes.len())?;
    out.extend(bytes);
    Ok(())
}

pub fn write(records: &BTreeMap<String, Record>) -> Result<Vec<u8>> {
    ensure!(!records.is_empty(), "No gameplay changes selected");
    let mut strings = Vec::<String>::new();
    let mut indices = BTreeMap::<String, usize>::new();
    let mut intern = |s: &str| -> usize {
        *indices.entry(s.to_owned()).or_insert_with(|| {
            let i = strings.len();
            strings.push(s.to_owned());
            i
        })
    };
    let mut blobs = Vec::new();
    let mut table = Vec::new();
    for (name, rec) in records {
        word(&mut table, intern(name))?;
        string(&mut table, &rec.class)?;
        let mut raw = Vec::new();
        for (key, field) in &rec.fields {
            raw.extend_from_slice(&field.kind.to_le_bytes());
            raw.extend_from_slice(&u16::try_from(field.values.len())?.to_le_bytes());
            word(&mut raw, intern(key))?;
            for value in &field.values {
                let bits = match (field.kind, value) {
                    (2, Value::Text(s)) => u32::try_from(intern(s))?,
                    (1, Value::Number(v)) if v.is_finite() && (*v as f32).is_finite() => {
                        (*v as f32).to_bits()
                    }
                    (0 | 3, Value::Number(v))
                        if v.fract() == 0.0 && *v >= i32::MIN as f64 && *v <= i32::MAX as f64 =>
                    {
                        (*v as i32) as u32
                    }
                    _ => bail!("Invalid value for ARZ type {}", field.kind),
                };
                raw.extend_from_slice(&bits.to_le_bytes());
            }
        }
        let packed = lz4_flex::block::compress(&raw);
        word(&mut table, blobs.len())?;
        word(&mut table, packed.len())?;
        word(&mut table, raw.len())?;
        table.extend_from_slice(&rec.time.to_le_bytes());
        blobs.extend(packed);
    }
    let mut names = Vec::new();
    word(&mut names, strings.len())?;
    for s in &strings {
        string(&mut names, s)?;
    }
    let mut out = vec![2, 0, 3, 0];
    word(&mut out, 24 + blobs.len())?;
    word(&mut out, table.len())?;
    word(&mut out, records.len())?;
    word(&mut out, 24 + blobs.len() + table.len())?;
    word(&mut out, names.len())?;
    out.extend(blobs);
    out.extend(table);
    out.extend(names);
    // The game's builder appends a 16-byte tool checksum; the runtime does not
    // require it for partial override databases. Validate with ArchiveTool too.
    out.extend_from_slice(&[0; 16]);
    ensure!(
        read(&out, |_| true)? == *records,
        "Generated archive failed read-back verification"
    );
    Ok(out)
}
