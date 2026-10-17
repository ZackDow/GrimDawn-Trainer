//! Opt-in live connection check; all modifiers remain at their original values.
//! cargo run -p grim-core --example live_check -- GAME_DIR RUNTIME_DIR
use anyhow::{ensure, Context, Result};
use grim_core::live::{Client, Modifiers, Runner};
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let game = PathBuf::from(args.next().context("Supply the game installation folder")?);
    let runtime = PathBuf::from(args.next().context("Supply the runtime resource folder")?);
    let mut client = Client::connect(&game, &runtime, &Runner::default())?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let ready = loop {
        let status = client.poll()?;
        ensure!(status["state"] != "error", "Runtime error: {status}");
        if status["state"] == "ready" {
            break status;
        }
        ensure!(Instant::now() < deadline, "Connection timed out: {status}");
        thread::sleep(Duration::from_millis(250));
    };
    client.set(Modifiers::default())?;
    // No UI polling: the native heartbeat must independently keep the lease alive.
    thread::sleep(Duration::from_secs(4));
    let active = client.poll()?;
    ensure!(
        active["state"] == "ready",
        "Native heartbeat failed: {active}"
    );
    ensure!(
        active["frames"].as_u64() > ready["frames"].as_u64(),
        "Game updates did not advance"
    );
    let restored = client.disconnect();
    ensure!(
        restored["restored"] == true,
        "Reset was not confirmed: {restored}"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"connected": ready, "afterFourSecondsWithoutUi": active, "disconnect": restored})
        )?
    );
    Ok(())
}
