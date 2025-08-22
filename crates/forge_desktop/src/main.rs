#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use forge_api::{ForgeAPI, API};
use forge_infra::ForgeInfra;
use forge_services::ForgeServices;
use tauri::Manager;

type ForgeApi = ForgeAPI<ForgeServices<ForgeInfra>, ForgeInfra>;

#[tauri::command]
async fn list_agents(app_handle: tauri::AppHandle) -> Result<Vec<String>, String> {
    let api = app_handle.state::<ForgeApi>();
    let workflow = api.read_workflow(None).await.map_err(|e| e.to_string())?;
    let agents = workflow
        .agents
        .into_iter()
        .map(|a| a.id.to_string())
        .collect();
    Ok(agents)
}

fn main() {
    let api = ForgeAPI::init(false, std::env::current_dir().unwrap());

    tauri::Builder::default()
        .manage(api)
        .invoke_handler(tauri::generate_handler![list_agents])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
