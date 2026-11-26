mod commands;

use commands::{check_gate, has_provider_key, run_step, scan_pipeline, store_provider_key};
use speckit_core::TemplateStore;

#[tauri::command]
fn template_count() -> Result<usize, String> {
    let root = if let Ok(p) = std::env::var("AIX_SPECKIT_CONTENT") {
        p.into()
    } else {
        std::path::PathBuf::from("../../content/speckit")
    };
    let store = TemplateStore::load_verified(&root).map_err(|e| e.to_string())?;
    Ok(store.len())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            scan_pipeline,
            check_gate,
            run_step,
            store_provider_key,
            has_provider_key,
            template_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running aix desktop");
}
