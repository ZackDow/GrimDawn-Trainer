//! Opt-in diagnostic. Read-only unless --unlock PLAYER_ID DIFFICULTY NAME is supplied.
use anyhow::{ensure, Context, Result};
use grim_core::live::{Client, PortalTarget, Runner};
use serde_json::json;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 2 || (args.len() == 6 && args[2] == "--unlock"),
        "Use GAME RUNTIME [--unlock PLAYER_ID DIFFICULTY CHARACTER_NAME]"
    );
    let mut client = Client::connect(
        &PathBuf::from(&args[0]),
        &PathBuf::from(&args[1]),
        &Runner::default(),
    )?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let before = loop {
        let status = client.poll()?;
        ensure!(status["state"] != "error", "Runtime error: {status}");
        if status["state"] == "ready" && status["portalTotal"].as_u64().unwrap_or(0) > 0 {
            break status;
        }
        ensure!(
            Instant::now() < deadline,
            "No active campaign character: {status}"
        );
        thread::sleep(Duration::from_millis(250));
    };
    let mut after = before.clone();
    if args.len() == 6 {
        let difficulty = args[4].parse::<u32>().context("Invalid difficulty")?;
        let target = PortalTarget {
            player: before["playerCookie"]
                .as_str()
                .context("No character")?
                .into(),
            player_id: args[3].clone(),
            character_name: args[5].clone(),
            session_id: before["sessionId"]
                .as_str()
                .context("No connection")?
                .into(),
            difficulty,
        };
        let sequence = client.unlock_portals(&target)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            after = client.poll()?;
            ensure!(after["state"] != "error", "Runtime error: {after}");
            if after["acknowledged"].as_i64() == Some(sequence as i64) {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "Portal action timed out: {after}"
            );
            thread::sleep(Duration::from_millis(250));
        }
        ensure!(after["commandResult"] == 0, "Portal action failed: {after}");
    }
    let disconnect = client.disconnect();
    ensure!(
        disconnect["restored"] == true,
        "Runtime reset was not confirmed: {disconnect}"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"before":before,"after":after,"disconnect":disconnect})
        )?
    );
    Ok(())
}
