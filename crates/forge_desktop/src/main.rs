#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use forge_api::{ForgeAPI, API};
use forge_domain::{ChatRequest, Event, McpConfig, McpServerConfig, Model, ModelId, Scope};
use forge_infra::ForgeInfra;
use forge_services::ForgeServices;
use tauri::{Manager, Emitter};
use tokio_stream::StreamExt;

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

#[tauri::command]
async fn get_mcp_config(app_handle: tauri::AppHandle) -> Result<McpConfig, String> {
    let api = app_handle.state::<ForgeApi>();
    api.read_mcp_config().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_mcp_server(
    app_handle: tauri::AppHandle,
    name: String,
    command: String,
    args: Vec<String>,
) -> Result<(), String> {
    let api = app_handle.state::<ForgeApi>();
    let mut config = api.read_mcp_config().await.map_err(|e| e.to_string())?;

    let server = McpServerConfig::new_stdio(command, args, None);
    config.mcp_servers.insert(name, server);

    api.write_mcp_config(&Scope::User, &config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_mcp_server(app_handle: tauri::AppHandle, name: String) -> Result<(), String> {
    let api = app_handle.state::<ForgeApi>();
    let mut config = api.read_mcp_config().await.map_err(|e| e.to_string())?;

    config.mcp_servers.remove(&name);

    api.write_mcp_config(&Scope::User, &config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_models(app_handle: tauri::AppHandle) -> Result<Vec<Model>, String> {
    let api = app_handle.state::<ForgeApi>();
    api.models().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_model(app_handle: tauri::AppHandle, model_id: String) -> Result<(), String> {
    let api = app_handle.state::<ForgeApi>();
    api.update_workflow(None, |workflow| {
        workflow.model = Some(ModelId::new(model_id));
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn send_chat_message(
    app_handle: tauri::AppHandle,
    message: String,
) -> Result<(), String> {
    let api = app_handle.state::<ForgeApi>();

    // For simplicity, we'll create a new conversation for each message.
    // In a real application, you would want to manage conversation state.
    let workflow = api.read_workflow(None).await.map_err(|e| e.to_string())?;
    let conversation = api
        .init_conversation(workflow)
        .await
        .map_err(|e| e.to_string())?;
    let conversation_id = conversation.id;

    let event = Event::new(
        "user_task_init",
        Some(serde_json::json!({ "task": message })),
    );
    let chat_request = ChatRequest::new(event, conversation_id);

    let mut stream = api.chat(chat_request).await.map_err(|e| e.to_string())?;

    while let Some(response) = stream.next().await {
        let response_str =
            serde_json::to_string(&response.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        app_handle.emit("chat_response", response_str).unwrap();
    }

    Ok(())
}

fn main() {
    let api = ForgeAPI::init(false, std::env::current_dir().unwrap());

    tauri::Builder::default()
        .manage(api)
        .invoke_handler(tauri::generate_handler![
            list_agents,
            get_mcp_config,
            add_mcp_server,
            remove_mcp_server,
            list_models,
            set_model,
            send_chat_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
