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
    match tauri::async_runtime::spawn_blocking(|| {
        // 若 49620-49629 任一端口已被监听，说明服务已在运行，直接返回成功。
        for port in 49620..=49629 {
            let addr = format!("127.0.0.1:{port}");
            if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
                if std::net::TcpStream::connect_timeout(
                    &addr,
                    std::time::Duration::from_millis(150),
                )
                .is_ok()
                {
                    return CmdResult::ok("bridge-server 已在运行".to_string());
                }
            }
        }
        run_node_script("bridge-server.mjs")
    })
    .await
    {
        Ok(res) => res,
        Err(e) => CmdResult::err(format!("启动线程异常：{e}")),
    }
}

/// Run the module-pin extraction script (`node run_main.js`), wait for it to finish.
#[tauri::command(rename_all = "snake_case")]
async fn run_main_js() -> CmdResult<String> {
    match tauri::async_runtime::spawn_blocking(|| run_node_script_wait("run_main.js")).await {
        Ok(res) => res,
        Err(e) => CmdResult::err(format!("运行线程异常：{e}")),
    }
}

/// Spawn `node <script>` located in the app working directory.
/// `capture` 为 true 时接管 stdout/stderr，供需要等待执行结果的脚本使用。
fn spawn_node_script(script: &str, capture: bool) -> Result<std::process::Child, String> {
    use std::process::{Command, Stdio};

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = cwd.join(script);
    if !path.exists() {
        return Err(format!("未找到 {}", path.display()));
    }
    if !cwd.join("node_modules").join("ws").exists() {
        return Err("缺少 ws 依赖，请先在程序目录执行: npm install ws".to_string());
    }

    let mut cmd = Command::new("node");
    cmd.arg(&path).current_dir(&cwd);
    if capture {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW，避免弹出控制台窗口
    }

    cmd.spawn()
        .map_err(|e| format!("启动失败（请确认已安装 Node.js 且在 PATH 中）：{e}"))
}

/// 快速冒烟检测：最多等 1 秒，进程仍存活即视为已启动（适用于常驻服务，如 bridge-server）。
fn run_node_script(script: &str) -> CmdResult<String> {
    let mut child = match spawn_node_script(script, false) {
        Ok(c) => c,
        Err(e) => return CmdResult::err(e),
    };

    // 最多等待 1 秒；若立即退出且非正常退出则说明有问题（如 ws 未装、脚本报错）。
    let deadline = std::time::Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                return CmdResult::ok(format!("{script} 执行完成"));
            }
            return CmdResult::err(format!("{script} 启动后立即退出（状态 {status}）"));
        }
        if deadline.elapsed() >= std::time::Duration::from_millis(1000) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    CmdResult::ok(format!("{script} 已启动"))
}

/// 真正等待脚本执行完毕（最多 30 秒）再反馈最终结果（适用于一次性任务，如 run_main.js）。
fn run_node_script_wait(script: &str) -> CmdResult<String> {
    let mut child = match spawn_node_script(script, true) {
        Ok(c) => c,
        Err(e) => return CmdResult::err(e),
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = match child.wait_with_output() {
                    Ok(o) => o,
                    Err(e) => return CmdResult::err(format!("读取输出失败：{e}")),
                };
                if output.status.success() {
                    let mut msg = format!("{script} 执行完成");
                    if let Ok(s) = String::from_utf8(output.stdout) {
                        let s = s.trim();
                        if !s.is_empty() {
                            let tail: String = s
                                .chars()
                                .rev()
                                .take(300)
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect();
                            msg.push_str(&format!("：{tail}"));
                        }
                    }
                    return CmdResult::ok(msg);
                } else {
                    let mut msg = format!("{script} 运行失败（状态 {}）", output.status);
                    if let Ok(s) = String::from_utf8(output.stderr) {
                        let s = s.trim();
                        if !s.is_empty() {
                            msg.push_str(&format!("：{s}"));
                        }
                    }
                    return CmdResult::err(msg);
                }
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return CmdResult::err(format!("{script} 运行超时（30秒）"));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(e) => return CmdResult::err(format!("等待进程出错：{e}")),
        }
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
            extract_selected_bom,
            start_bridge_server,
            run_main_js
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
