mod engine;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Start the EasyEDA WebSocket bridge server (`node bridge-server.mjs`).
#[tauri::command(rename_all = "snake_case")]
async fn start_bridge_server() -> CmdResult<String> {
    match tauri::async_runtime::spawn_blocking(run_bridge_server).await {
        Ok(res) => res,
        Err(e) => CmdResult::err(format!("启动线程异常：{e}")),
    }
}

fn run_bridge_server() -> CmdResult<String> {
    use std::process::Command;

    // 若 49620-49629 任一端口已被监听，说明服务已在运行，直接返回成功。
    for port in 49620..=49629 {
        let addr = format!("127.0.0.1:{port}");
        if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
            if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(150))
                .is_ok()
            {
                return CmdResult::ok("bridge-server 已在运行".to_string());
            }
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let script = cwd.join("bridge-server.mjs");
    if !script.exists() {
        return CmdResult::err(format!("未找到 {}", script.display()));
    }
    if !cwd.join("node_modules").join("ws").exists() {
        return CmdResult::err("缺少 ws 依赖，请先在程序目录执行: npm install ws".to_string());
    }

    let mut cmd = Command::new("node");
    cmd.arg(&script).current_dir(&cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW，避免弹出控制台窗口
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CmdResult::err(format!(
                "启动失败（请确认已安装 Node.js 且在 PATH 中）：{e}"
            ));
        }
    };

    // 最多等待 1 秒；若进程立即退出则说明有问题（如 ws 未装、端口被占用）。
    let deadline = std::time::Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return CmdResult::err(format!(
                "bridge-server 启动后立即退出（状态 {status}），请确认 ws 已安装、端口未被占用"
            ));
        }
        if deadline.elapsed() >= std::time::Duration::from_millis(1000) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    CmdResult::ok("bridge-server 已启动".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            read_xlsx_preview,
            extract_selected_bom,
            start_bridge_server
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
