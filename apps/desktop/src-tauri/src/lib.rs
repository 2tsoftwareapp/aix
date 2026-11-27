mod commands;

use commands::{
    delete_provider_key,
    list_models,
    list_vendor_configs,
    cancel_run, check_gate, flip_checklist_item, has_provider_key, read_artifact, respond_approval,
    run_step, scan_pipeline, store_provider_key, write_artifact, ApprovalRegistry, RunCancellation,
};
use speckit_core::TemplateStore;
use tauri::Manager as _;

#[tauri::command]
fn template_count() -> Result<usize, String> {
    let root = if let Ok(p) = std::env::var("AIX_SPECKIT_CONTENT") {
        p.into()
    } else {
        std::path::PathBuf::from("../speckit-templates")
    };
    let store = TemplateStore::load_verified(&root).map_err(|e| e.to_string())?;
    Ok(store.len())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Embedded ai-memory lives in the app-private data dir: it does not
            // share state with any machine-wide ai-memory service by design.
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("aix-memory-fallback"))
                .join("memory");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let bridge = memory_bridge::MemoryBridge::open(&dir).map_err(|e| e.to_string())?;
            app.manage(std::sync::Arc::new(bridge));
            Ok(())
        })
        .manage(ApprovalRegistry::default())
        .manage(RunCancellation::default())
        .invoke_handler(tauri::generate_handler![
            scan_pipeline,
            check_gate,
            run_step,
            respond_approval,
            cancel_run,
            store_provider_key,
            has_provider_key,
            read_artifact,
            write_artifact,
            flip_checklist_item,
            template_count,
            delete_provider_key,
            list_models,
            list_vendor_configs,
            commands::is_git_repo,])
        .run(tauri::generate_context!())
        .expect("error while running aix desktop");
}
