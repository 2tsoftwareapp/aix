use speckit_core::{PipelineState, TemplateStore};

#[tauri::command]
fn scan_pipeline(project_root: String) -> Result<PipelineState, String> {
    Ok(PipelineState::scan(&project_root))
}

#[tauri::command]
fn template_count() -> Result<usize, String> {
    let root = template_root();
    let store = TemplateStore::load_verified(&root).map_err(|e| e.to_string())?;
    Ok(store.len())
}

fn template_root() -> std::path::PathBuf {
    // Dev: repo content/speckit. Prod: bundled resource.
    if let Ok(p) = std::env::var("AIX_SPECKIT_CONTENT") {
        return p.into();
    }
    std::path::PathBuf::from("../../content/speckit")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![scan_pipeline, template_count])
        .run(tauri::generate_context!())
        .expect("error while running aix desktop");
}
