use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};
#[derive(Parser)]
#[command(version, about = "Grim Dawn Trainer file tools")]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Scan,
    RetireLegacy {
        game: PathBuf,
    },
    DisableRules {
        game: PathBuf,
    },
    BuildRules {
        game: PathBuf,
        profile: PathBuf,
        output: PathBuf,
    },
    InspectSave {
        path: PathBuf,
    },
    InspectDb {
        path: PathBuf,
        #[arg(default_value = "records/game/gameengine.dbr")]
        pattern: String,
    },
    EditCopy {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long)]
        skill: u32,
        #[arg(long)]
        attribute: u32,
        #[arg(long)]
        devotion: u32,
        #[arg(long)]
        devotion_cap: u32,
    },
}
fn main() -> Result<()> {
    match Args::parse().command {
        Command::RetireLegacy { game } => println!(
            "{}",
            serde_json::to_string_pretty(&grim_core::storage::retire_unused_legacy(
                &grim_core::storage::data_dir()?,
                &game
            )?)?
        ),
        Command::DisableRules { game } => {
            let mut service = grim_core::service::Service::new(grim_core::storage::data_dir()?);
            let plan = service.preview_disable(&game)?;
            let receipt = service.apply(&plan.id)?;
            println!("{}", serde_json::to_string_pretty(&receipt)?);
        }
        Command::Scan => println!(
            "{}",
            serde_json::to_string_pretty(&grim_core::discovery::scan(None, None))?
        ),
        Command::BuildRules {
            game,
            profile,
            output,
        } => {
            let profile = serde_json::from_slice(&fs::read(profile)?)?;
            let data = grim_core::rules::load(&game)?;
            let built = grim_core::rules::build(&data, &profile)?;
            grim_core::rules::verify_sources(&built.summary.sources)?;
            grim_core::storage::export_new(&output, &built.bytes)?;
            let manifest = output.with_extension("manifest.json");
            grim_core::storage::export_new(&manifest, &serde_json::to_vec_pretty(&built.summary)?)?;
            println!("{}", serde_json::to_string_pretty(&built.summary.counts)?);
        }
        Command::InspectSave { path } => println!(
            "{}",
            serde_json::to_string_pretty(&grim_core::gdc::read(&fs::read(path)?)?)?
        ),
        Command::InspectDb { path, pattern } => println!(
            "{}",
            serde_json::to_string_pretty(
                &grim_core::arz::read(&fs::read(path)?, |n| n.contains(&pattern))?
            )?
        ),
        Command::EditCopy {
            source,
            destination,
            skill,
            attribute,
            devotion,
            devotion_cap,
        } => {
            use std::io::Write;
            let (_, data) = grim_core::gdc::edit(
                &fs::read(source)?,
                grim_core::gdc::Points {
                    skill,
                    attribute,
                    devotion,
                },
                devotion_cap,
            )?;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            f.write_all(&data)?;
            f.sync_all()?;
            println!("Verified edited copy written");
        }
    }
    Ok(())
}
