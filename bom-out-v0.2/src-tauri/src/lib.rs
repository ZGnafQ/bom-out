mod engine;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdResult<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> CmdResult<T> {
    fn ok(data: T) -> Self {
        CmdResult {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(error: impl Into<String>) -> Self {
        CmdResult {
            ok: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
fn read_xlsx_preview(path: String) -> CmdResult<engine::XlsxPreview> {
    match engine::read_xlsx_preview(&path) {
        Ok(data) => CmdResult::ok(data),
        Err(e) => CmdResult::err(e),
    }
}

#[tauri::command(rename_all = "snake_case")]
fn extract_selected_bom(pin_path: String, bom_path: String) -> CmdResult<engine::ExtractResult> {
    match engine::extract_selected_bom(&pin_path, &bom_path) {
        Ok(data) => CmdResult::ok(data),
        Err(e) => CmdResult::err(e),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            read_xlsx_preview,
            extract_selected_bom
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
