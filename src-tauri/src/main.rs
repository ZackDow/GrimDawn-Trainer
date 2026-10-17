#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use grim_core::{desktop, live, model, storage};
use std::{path::PathBuf, sync::Mutex};
type Reply<T> = Result<T, String>;
struct LiveState {
    client: Mutex<Option<live::Client>>,
    resources: PathBuf,
    data: PathBuf,
}
#[tauri::command]
async fn live_connect(
    state: tauri::State<'_, LiveState>,
    game: PathBuf,
    runner: live::Runner,
) -> Reply<serde_json::Value> {
    storage::retire_unused_legacy(&storage::data_dir().map_err(|e| e.to_string())?, &game)
        .map_err(|e| format!("{e:#}"))?;
    let mut client = state.client.lock().map_err(|e| e.to_string())?;
    if let Some(mut old) = client.take() {
        old.disconnect();
    }
    let new =
        live::Client::connect(&game, &state.resources, &runner).map_err(|e| format!("{e:#}"))?;
    *client = Some(new);
    client.as_mut().unwrap().poll().map_err(|e| e.to_string())
}
#[tauri::command]
fn live_poll(state: tauri::State<'_, LiveState>) -> Reply<serde_json::Value> {
    let mut client = state.client.lock().map_err(|e| e.to_string())?;
    match client.as_mut() {
        Some(c) => c.poll().map_err(|e| e.to_string()),
        None => Ok(serde_json::json!({"state":"disconnected","error":""})),
    }
}
#[tauri::command]
fn live_set(state: tauri::State<'_, LiveState>, modifiers: live::Modifiers) -> Reply<()> {
    state
        .client
        .lock()
        .map_err(|e| e.to_string())?
        .as_mut()
        .ok_or("Connect first")?
        .set(modifiers)
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn live_disconnect(state: tauri::State<'_, LiveState>) -> Reply<serde_json::Value> {
    let mut client = state.client.lock().map_err(|e| e.to_string())?;
    Ok(client
        .take()
        .map(|mut c| c.disconnect())
        .unwrap_or_else(|| serde_json::json!({"state":"disconnected","restored":true})))
}
#[tauri::command]
fn live_grant(
    state: tauri::State<'_, LiveState>,
    player: String,
    points: model::Points,
) -> Reply<i32> {
    state
        .client
        .lock()
        .map_err(|e| e.to_string())?
        .as_mut()
        .ok_or("Connect first")?
        .grant(&player, points)
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn live_unlock_portals(
    state: tauri::State<'_, LiveState>,
    target: live::PortalTarget,
) -> Reply<i32> {
    state
        .client
        .lock()
        .map_err(|e| e.to_string())?
        .as_mut()
        .ok_or("Connect first")?
        .unlock_portals(&target)
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn home(state: tauri::State<'_, LiveState>) -> Reply<desktop::Home> {
    let data = state.data.clone();
    tauri::async_runtime::spawn_blocking(move || desktop::home(&data).map_err(|e| format!("{e:#}")))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn configure(
    state: tauri::State<'_, LiveState>,
    game: Option<PathBuf>,
) -> Reply<desktop::Home> {
    {
        let mut client = state.client.lock().map_err(|e| e.to_string())?;
        if let Some(c) = client.as_mut() {
            let status = c.poll().map_err(|e| e.to_string())?;
            if status["state"] == "ready" || status["state"] == "connecting" {
                return Err("Disconnect before changing the game folder".into());
            }
        }
    }
    let data = state.data.clone();
    tauri::async_runtime::spawn_blocking(move || {
        desktop::configure(&data, game).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
fn main() {
    use tauri::Manager;
    let data = storage::data_dir().expect("Application data folder unavailable");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                if let Some(state) = window.try_state::<LiveState>() {
                    if let Ok(mut client) = state.client.lock() {
                        if let Some(mut client) = client.take() {
                            client.disconnect();
                        }
                    }
                }
            }
        })
        .setup(move |app| {
            let exe = std::env::current_exe()?;
            let candidates = [
                app.path().resource_dir()?.join("runtime"),
                exe.parent().unwrap().join("runtime"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../runtime/bin"),
            ];
            let resources = live::resources(&candidates)?;
            app.manage(LiveState {
                client: Mutex::new(None),
                resources,
                data,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            home,
            configure,
            live_connect,
            live_poll,
            live_set,
            live_disconnect,
            live_grant,
            live_unlock_portals
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Grim Dawn Trainer");
}
