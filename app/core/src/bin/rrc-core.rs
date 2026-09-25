//! Command-line access to the core, for development, support and the parity
//! check against the Python companion. Players use the app, not this.

use rrc_core::credentials::platform_store;
use rrc_core::game::{saved_files, wow_running};
use rrc_core::queue::Queue;
use rrc_core::save::{compact_one, scan_one};
use rrc_core::upload::{prepare_batch, redeem_code, upload_all, Https};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage: rrc-core [--game DIR] [--state DIR] [--delay SECONDS] <command>

commands:
  scan              copy observations from completed game saves
  status            show the local queue
  preview [N]       newest N records: kind and date, no text
  batch             print the next upload batch exactly as it would be sent
  pair CODE [--upload]  connect this PC with a code; --upload sends everything straight after
  upload            send every pending record
  rollover --yes    free the addon's space once WoW is closed
  forget --yes      delete the local queue; the game save is untouched";

fn default_state() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("RestedRealmCollector").join("Companion")
}

fn default_game() -> PathBuf {
    let base = std::env::var_os("PROGRAMFILES(X86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    base.join("World of Warcraft").join("_classic_beta_")
}

fn run() -> Result<bool, String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut game = default_game();
    let mut state = default_state();
    let mut delay = Duration::from_secs(1);
    while args.first().is_some_and(|a| a.starts_with("--")) && args.len() >= 2 {
        let value = args.remove(1);
        match args.remove(0).as_str() {
            "--game" => game = value.into(),
            "--state" => state = value.into(),
            "--delay" => delay = Duration::from_secs_f64(value.parse().map_err(|_| "bad --delay")?),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    let Some(command) = args.first().cloned() else {
        return Err(USAGE.into());
    };
    let confirmed = args.iter().any(|a| a == "--yes");
    let confirmed_upload = args.iter().any(|a| a == "--upload");
    let mut queue = Queue::open(&state).map_err(|e| e.to_string())?;
    match command.as_str() {
        "scan" => {
            let paths = saved_files(&game);
            if paths.is_empty() {
                println!("No RestedRealm addon save found in {}.", game.display());
                return Ok(false);
            }
            let mut worked = false;
            for path in paths {
                match scan_one(&mut queue, &path, delay) {
                    // Do not print an account folder name or raw save text.
                    Err(e) => eprintln!("Save skipped: {e}"),
                    Ok(r) if r.unchanged => {
                        worked = true;
                        println!("Save unchanged.");
                    }
                    Ok(r) => {
                        worked = true;
                        println!(
                            "Queued {} new, updated {} corrected; {} in save, {} addon drops.",
                            r.new,
                            r.updated,
                            r.records.unwrap_or(0),
                            r.dropped.unwrap_or(0)
                        );
                    }
                }
            }
            Ok(worked)
        }
        "status" => {
            let s = queue.status().map_err(|e| e.to_string())?;
            println!(
                "RestedRealm Companion {}: {} observations on this PC, {} waiting to upload, {} not accepted, {} save(s), {} addon drops.",
                rrc_core::VERSION, s.observations, s.pending, s.rejected, s.sources, s.dropped
            );
            Ok(true)
        }
        "preview" => {
            let limit = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(20);
            for row in queue.recent(limit).map_err(|e| e.to_string())? {
                let record: serde_json::Value = serde_json::from_str(&row.payload).unwrap_or_default();
                let kind = record.get("kind").and_then(|k| k.as_str()).unwrap_or("?");
                let when = record.get("observedAt").and_then(|t| t.as_i64()).unwrap_or(0);
                let sent = if row.rejected.is_some() {
                    "not accepted"
                } else if row.uploaded {
                    "uploaded"
                } else {
                    "waiting"
                };
                println!("{}  seq {}  {kind}  observedAt {when}  {sent}", &row.digest[..12], row.seq);
            }
            Ok(true)
        }
        "batch" => {
            println!("{}", prepare_batch(&queue, 50).map_err(|e| e.to_string())?.body());
            Ok(true)
        }
        "pair" => {
            let code = args.get(1).ok_or("pair needs the code from your account page")?;
            let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows PC".into());
            let store = platform_store();
            let device = redeem_code(&Https::default(), store.as_ref(), code, &name).map_err(|e| e.to_string())?;
            println!("Connected to RestedRealm as device {device}.");
            if confirmed_upload {
                let sent = upload_all(&mut queue, &Https::default(), store.as_ref()).map_err(|e| e.to_string())?;
                let status = queue.status().map_err(|e| e.to_string())?;
                println!("Uploaded {sent} observations; {} not accepted by RestedRealm.", status.rejected);
            }
            Ok(true)
        }
        "upload" => {
            let sent =
                upload_all(&mut queue, &Https::default(), platform_store().as_ref()).map_err(|e| e.to_string())?;
            println!("Uploaded {sent} observations.");
            Ok(true)
        }
        "rollover" if confirmed => {
            let mut worked = false;
            for path in saved_files(&game) {
                match compact_one(&queue, &path, delay, &wow_running) {
                    Ok(count) => {
                        worked = true;
                        println!("Freed {count} safely queued observations; a private backup was kept.");
                    }
                    Err(e) => eprintln!("Rollover skipped: {e}"),
                }
            }
            Ok(worked)
        }
        "forget" if confirmed => {
            queue.forget().map_err(|e| e.to_string())?;
            println!("Local queue deleted. The WoW save was not changed.");
            Ok(true)
        }
        "rollover" | "forget" => Err(format!("{command} requires --yes")),
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
