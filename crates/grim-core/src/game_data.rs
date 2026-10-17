use crate::{arz, discovery};
use anyhow::{ensure, Context, Result};
use std::{fs, path::Path};

pub fn devotion_cap(game: &Path) -> Result<u32> {
    let mut cap = None;
    for path in discovery::archives(game)? {
        let records = arz::read(&fs::read(path)?, |n| {
            n == "records/creatures/pc/playerlevels.dbr"
        })?;
        if let Some(r) = records.values().next() {
            cap = r.number("maxDevotionPoints");
        }
    }
    let cap = cap.context("The installed game does not define a devotion limit")?;
    ensure!(
        (1. ..=10_000.).contains(&cap) && cap.fract() == 0.,
        "Invalid devotion limit"
    );
    Ok(cap as u32)
}
