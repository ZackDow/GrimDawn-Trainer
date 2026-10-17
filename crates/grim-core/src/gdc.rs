//! Lossless, width-aware GDC editing for the current v8/v11 save layout.
//! Every primitive, nested block, checksum and end position is validated.
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub use crate::model::Points;
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub name: String,
    pub class: String,
    pub level: u32,
    pub hardcore: bool,
    pub points: Points,
    pub total_devotion: u32,
    pub portal_counts: [u32; 3],
    pub save_id: String,
    pub blocks: BTreeMap<u32, u32>,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Byte(u8),
    Word(u32),
    Size(u32),
    Check,
}

struct Cipher {
    state: u32,
    table: [u32; 256],
}
impl Cipher {
    fn new(seed: u32) -> Self {
        let mut x = seed;
        let table = std::array::from_fn(|_| {
            x = x.rotate_right(1).wrapping_mul(39916801);
            x
        });
        Self { state: seed, table }
    }
    fn absorb(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.state ^= self.table[*b as usize];
        }
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    cipher: Cipher,
    tape: Vec<Token>,
    points_at: Option<usize>,
    portal_counts: [u32; 3],
    blocks: BTreeMap<u32, u32>,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).context("Save offset overflow")?;
        let out = self
            .bytes
            .get(self.pos..end)
            .context("Truncated save file")?;
        self.pos = end;
        Ok(out)
    }
    fn byte(&mut self) -> Result<u8> {
        let raw = self.take(1)?[0];
        let plain = raw ^ self.cipher.state as u8;
        self.cipher.absorb(&[raw]);
        self.tape.push(Token::Byte(plain));
        Ok(plain)
    }
    fn word(&mut self) -> Result<u32> {
        let raw: [u8; 4] = self.take(4)?.try_into()?;
        let plain = u32::from_le_bytes(raw) ^ self.cipher.state;
        self.cipher.absorb(&raw);
        self.tape.push(Token::Word(plain));
        Ok(plain)
    }
    fn words(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.word()?;
        }
        Ok(())
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        (0..n).map(|_| self.byte()).collect()
    }
    fn count(&mut self) -> Result<usize> {
        let n = self.word()? as usize;
        ensure!(
            n <= 100_000 && n <= self.bytes.len() - self.pos,
            "Invalid array length {n}"
        );
        Ok(n)
    }
    fn string(&mut self, wide: bool) -> Result<String> {
        let n = self.count()?;
        let raw = self.bytes(n * if wide { 2 } else { 1 })?;
        if wide {
            let units: Vec<_> = raw
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(String::from_utf16(&units).context("Invalid character name")?)
        } else {
            Ok(raw.into_iter().map(char::from).collect())
        }
    }
    fn strings(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.string(false)?;
        }
        Ok(())
    }
    fn check(&mut self) -> Result<()> {
        let value = u32::from_le_bytes(self.take(4)?.try_into()?);
        ensure!(
            value == self.cipher.state,
            "Save checksum mismatch at {}",
            self.pos - 4
        );
        self.tape.push(Token::Check);
        Ok(())
    }
    fn start(&mut self) -> Result<(u32, usize)> {
        let id = self.word()?;
        let len = u32::from_le_bytes(self.take(4)?.try_into()?) ^ self.cipher.state;
        self.tape.push(Token::Size(len));
        let end = self
            .pos
            .checked_add(len as usize)
            .context("Block length overflow")?;
        ensure!(
            end.checked_add(4).is_some_and(|e| e <= self.bytes.len()),
            "Block {id} extends beyond save"
        );
        Ok((id, end))
    }
    fn finish(&mut self, id: u32, end: usize) -> Result<()> {
        ensure!(
            self.pos == end,
            "Unsupported block {id} layout: decoded to {}, expected {end}",
            self.pos
        );
        self.check()
    }
    fn item(&mut self) -> Result<()> {
        self.strings(5)?;
        self.word()?;
        self.strings(2)?;
        self.word()?;
        self.string(false)?;
        self.words(8)
    }
    fn item_array(&mut self, coordinates: bool) -> Result<()> {
        let n = self.count()?;
        for _ in 0..n {
            self.item()?;
            if coordinates {
                self.words(2)?;
            }
        }
        Ok(())
    }
    fn uid_lists(&mut self, lists: usize, extra: usize) -> Result<()> {
        for _ in 0..lists {
            let n = self.count()?;
            self.bytes(n * 16)?;
        }
        self.bytes(extra * 16)?;
        Ok(())
    }
    fn block(&mut self, id: u32, end: usize) -> Result<()> {
        let version = self.word()?;
        let accepted: &[u32] = match id {
            1 => &[5],
            2 | 8 => &[8],
            3 | 4 => &[11],
            5 | 6 | 7 | 12 | 15 => &[1],
            17 | 10 => &[2],
            13 => &[5],
            14 => &[7],
            16 => &[12],
            _ => &[],
        };
        ensure!(
            accepted.contains(&version),
            "Unsupported save block {id} version {version}; no file was changed"
        );
        ensure!(
            self.blocks.insert(id, version).is_none(),
            "Duplicate save block {id}"
        );
        match id {
            1 => {
                self.bytes(4)?;
                self.word()?;
                self.byte()?;
                self.word()?;
                self.bytes(4)?;
                self.string(false)?;
                let n = self.count()?;
                self.bytes(n)?;
            }
            2 => {
                self.words(2)?;
                self.points_at = Some(self.tape.len());
                self.words(9)?;
            }
            3 => {
                let has_data = self.byte()?;
                ensure!(has_data <= 1, "Invalid inventory flag");
                if has_data == 1 {
                    let n = self.count()?;
                    self.words(2)?;
                    for _ in 0..n {
                        let (i, e) = self.start()?;
                        ensure!(i == 0, "Unknown inventory sub-block");
                        self.byte()?;
                        self.item_array(true)?;
                        self.finish(i, e)?;
                    }
                    for count in [12, 2, 2] {
                        self.byte()?;
                        for _ in 0..count {
                            self.item()?;
                            self.byte()?;
                        }
                    }
                }
            }
            4 => {
                let n = self.count()?;
                for _ in 0..n {
                    let (i, e) = self.start()?;
                    ensure!(i == 0, "Unknown stash sub-block");
                    self.words(2)?;
                    self.item_array(true)?;
                    self.words(5)?;
                    self.finish(i, e)?;
                }
            }
            5 => self.uid_lists(3, 3)?, // Respawn lists and current respawn IDs.
            6 => {
                // Player::SaveNewFormatData writes teleport vectors in block 6.
                for d in 0..3 {
                    let count = self.count()?;
                    self.portal_counts[d] = count as u32;
                    self.bytes(count * 16)?;
                }
            }
            7 => self.uid_lists(3, 0)?,
            17 => self.uid_lists(6, 0)?,
            8 => {
                let n = self.count()?;
                for _ in 0..n {
                    self.string(false)?;
                    self.word()?;
                    self.bytes(2)?;
                    self.words(3)?;
                    self.bytes(2)?;
                    self.strings(2)?;
                }
                self.words(3)?;
                let items = self.count()?;
                // Item-skill payloads have changed across patches. Refuse these
                // until an independent v8 fixture confirms their exact widths.
                ensure!(
                    items == 0,
                    "This save contains item-skill bindings not yet supported for writing"
                );
                self.word()?;
            }
            10 => {
                for _ in 0..3 {
                    let n = self.count()?;
                    self.strings(n)?;
                }
            }
            12 => {
                let n = self.count()?;
                self.strings(n)?;
            }
            13 => {
                self.word()?;
                let n = self.count()?;
                for _ in 0..n {
                    self.bytes(2)?;
                    self.words(3)?;
                }
            }
            14 => {
                self.byte()?;
                self.word()?;
                self.byte()?;
                for _ in 0..5 {
                    self.strings(2)?;
                    self.byte()?;
                }
                self.words(3)?;
                while self.pos + 8 < end {
                    match self.word()? {
                        0 => {
                            self.string(false)?;
                            self.byte()?;
                            self.string(false)?;
                            self.word()?;
                        }
                        4 => {
                            self.strings(3)?;
                            self.string(true)?;
                        }
                        1 | 2 | 3 | u32::MAX => (),
                        kind => bail!("Unsupported hotbar slot type {kind}"),
                    }
                }
                self.words(2)?;
            }
            15 => {
                let n = self.count()?;
                self.words(n)?;
            }
            16 => {
                self.words(12)?;
                for _ in 0..3 {
                    self.string(false)?;
                    self.words(2)?;
                    self.strings(2)?;
                }
                self.words(19)?;
                let n = self.count()?;
                for _ in 0..n {
                    self.string(false)?;
                    self.word()?;
                }
                self.words(2)?;
                self.byte()?;
                self.words(4)?;
            }
            _ => unreachable!(),
        }
        self.finish(id, end)
    }
}

fn replay(seed: u32, tape: &[Token]) -> Vec<u8> {
    let mut cipher = Cipher::new(seed);
    let mut out = (seed ^ 0x55555555).to_le_bytes().to_vec();
    for token in tape {
        match token {
            Token::Byte(v) => {
                let bytes = [*v ^ cipher.state as u8];
                out.extend(bytes);
                cipher.absorb(&bytes);
            }
            Token::Word(v) => {
                let bytes = (*v ^ cipher.state).to_le_bytes();
                out.extend(bytes);
                cipher.absorb(&bytes);
            }
            Token::Size(v) => out.extend((*v ^ cipher.state).to_le_bytes()),
            Token::Check => out.extend(cipher.state.to_le_bytes()),
        }
    }
    out
}
struct Parsed {
    character: Character,
    seed: u32,
    tape: Vec<Token>,
    points_at: usize,
}
fn parse(bytes: &[u8]) -> Result<Parsed> {
    ensure!(
        bytes.len() >= 32 && bytes.len() <= 64_000_000,
        "Invalid save size"
    );
    let seed = u32::from_le_bytes(bytes[..4].try_into()?) ^ 0x55555555;
    let mut r = Reader {
        bytes,
        pos: 4,
        cipher: Cipher::new(seed),
        tape: Vec::new(),
        points_at: None,
        portal_counts: [0; 3],
        blocks: BTreeMap::new(),
    };
    ensure!(r.word()? == 0x58434447, "Not a Grim Dawn character save");
    ensure!(r.word()? == 2, "Unsupported save header version");
    let name = r.string(true)?;
    r.byte()?;
    let class = r.string(false)?;
    let level = r.word()?;
    let hardcore = r.byte()? != 0;
    r.byte()?;
    r.check()?;
    ensure!(r.word()? == 8, "Unsupported save data version");
    let save_id = r
        .bytes(16)?
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    while r.pos < bytes.len() {
        let (id, end) = r.start()?;
        r.block(id, end)
            .with_context(|| format!("Save block {id}"))?;
    }
    let points_at = r.points_at.context("Save has no character attributes")?;
    let value = |i: usize| match r.tape.get(points_at + i) {
        Some(Token::Word(v)) => Ok(*v),
        _ => bail!("Invalid points layout"),
    };
    let points = Points {
        attribute: value(0)?,
        skill: value(1)?,
        devotion: value(2)?,
    };
    let total_devotion = value(3)?;
    ensure!(
        total_devotion >= points.devotion,
        "Devotion totals are inconsistent"
    );
    ensure!(
        replay(seed, &r.tape) == bytes,
        "Save failed lossless round trip"
    );
    Ok(Parsed {
        character: Character {
            name,
            class,
            level,
            hardcore,
            points,
            total_devotion,
            portal_counts: r.portal_counts,
            save_id,
            blocks: r.blocks,
            sha256: crate::digest(bytes),
        },
        seed,
        tape: r.tape,
        points_at,
    })
}
pub fn read(bytes: &[u8]) -> Result<Character> {
    Ok(parse(bytes)?.character)
}

pub fn edit(bytes: &[u8], target: Points, devotion_cap: u32) -> Result<(Character, Vec<u8>)> {
    ensure!(
        target.skill <= 10_000 && target.attribute <= 10_000,
        "Skill and attribute balances must be between 0 and 10000"
    );
    let mut p = parse(bytes)?;
    let spent = p.character.total_devotion - p.character.points.devotion;
    let total = spent
        .checked_add(target.devotion)
        .context("Devotion overflow")?;
    ensure!(total<=devotion_cap,"Devotion total would be {total}, above the current game limit {devotion_cap} ({spent} already spent)");
    let original = p.tape.clone();
    for (i, v) in [target.attribute, target.skill, target.devotion, total]
        .iter()
        .enumerate()
    {
        p.tape[p.points_at + i] = Token::Word(*v);
    }
    let output = replay(p.seed, &p.tape);
    let verified = parse(&output)?;
    ensure!(
        verified.character.points == target && verified.character.total_devotion == total,
        "Points failed read-back verification"
    );
    let allowed: BTreeSet<_> = (p.points_at..p.points_at + 4).collect();
    ensure!(
        original.len() == verified.tape.len()
            && original
                .iter()
                .zip(&verified.tape)
                .enumerate()
                .all(|(i, (a, b))| allowed.contains(&i) || a == b),
        "An unrelated save field changed"
    );
    Ok((verified.character, output))
}
