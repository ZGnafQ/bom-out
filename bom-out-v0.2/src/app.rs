#![allow(non_snake_case)]

use dioxus::prelude::*;
use js_sys::{Array, Reflect};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

static CSS: Asset = asset!("/assets/styles.css");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: js_sys::Function) -> JsValue;
}

#[derive(Deserialize, Clone)]
pub struct CmdResult<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
pub struct XlsxPreview {
    #[allow(dead_code)]
    pub sheet: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Deserialize, Clone)]
pub struct ModelResult {
    pub name: String,
    pub kept_rows: Vec<Vec<String>>,
    pub diff_rows: Vec<Vec<String>>,
    #[allow(dead_code)]
    pub deleted: Vec<String>,
    #[allow(dead_code)]
    pub kept: Vec<String>,
}

#[derive(Deserialize, Clone, Default)]
pub struct ExtractResult {
    pub models: Vec<ModelResult>,
    #[allow(dead_code)]
    pub bom_designator_col: usize,
    pub output_path: String,
}

#[derive(Serialize)]
struct PathArgs<'a> {
    path: &'a str,
}

#[derive(Serialize)]
struct ExtractArgs<'a> {
    pin_path: &'a str,
    bom_path: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileFilter {
    name: String,
    extensions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenOptions {
    multiple: bool,
    title: String,
    filters: Vec<FileFilter>,
}

#[derive(Serialize)]
struct DialogArgs {
    options: OpenOptions,
}

async fn load_preview(
    path: String,
    mut target: Signal<Option<XlsxPreview>>,
    mut status: Signal<String>,
) {
    let args = serde_wasm_bindgen::to_value(&PathArgs { path: &path }).unwrap_or(JsValue::UNDEFINED);
    let val = invoke("read_xlsx_preview", args).await;
    match serde_wasm_bindgen::from_value::<CmdResult<XlsxPreview>>(val) {
        Ok(res) if res.ok => {
            if let Some(data) = res.data {
                target.set(Some(data));
                status.set(String::new());
            } else {
                target.set(None);
            }
        }
        Ok(res) => {
            target.set(None);
            status.set(res.error.unwrap_or_else(|| "读取文件失败".into()));
        }
        Err(_) => {
            target.set(None);
            status.set("读取文件失败".into());
        }
    }
}

async fn pick_file(
    zone: u8,
    mut pin_path: Signal<String>,
    mut bom_path: Signal<String>,
    pin_preview: Signal<Option<XlsxPreview>>,
    bom_preview: Signal<Option<XlsxPreview>>,
    mut extract: Signal<Option<ExtractResult>>,
    status: Signal<String>,
) {
    let title = if zone == 0 {
        "选择引脚定义文件"
    } else {
        "选择完整BOM文件"
    };
    let options = OpenOptions {
        multiple: false,
        title: title.into(),
        filters: vec![FileFilter {
            name: "Excel 文件".into(),
            extensions: vec!["xlsx".into()],
        }],
    };
    let args = serde_wasm_bindgen::to_value(&DialogArgs { options }).unwrap_or(JsValue::UNDEFINED);
    let val = invoke("plugin:dialog|open", args).await;
    if let Some(p) = val.as_string() {
        if p.is_empty() {
            return;
        }
        extract.set(None);
        if zone == 0 {
            pin_path.set(p.clone());
            wasm_bindgen_futures::spawn_local(load_preview(p, pin_preview, status));
        } else {
            bom_path.set(p.clone());
            wasm_bindgen_futures::spawn_local(load_preview(p, bom_preview, status));
        }
    }
}

async fn run_extract(
    pin_path: Signal<String>,
    bom_path: Signal<String>,
    mut extract: Signal<Option<ExtractResult>>,
    mut active_model: Signal<usize>,
    mut busy: Signal<bool>,
    mut status: Signal<String>,
) {
    let pin = pin_path.read().clone();
    let bom = bom_path.read().clone();
    if pin.is_empty() || bom.is_empty() {
        status.set("请先拖入或选择引脚定义文件与完整BOM文件".into());
        return;
    }
    busy.set(true);
    status.set("正在生成选贴BOM...".into());
    let args = serde_wasm_bindgen::to_value(&ExtractArgs {
        pin_path: &pin,
        bom_path: &bom,
    })
    .unwrap_or(JsValue::UNDEFINED);
    let val = invoke("extract_selected_bom", args).await;
    busy.set(false);
    match serde_wasm_bindgen::from_value::<CmdResult<ExtractResult>>(val) {
        Ok(res) if res.ok => {
            if let Some(data) = res.data {
                let out = data.output_path.clone();
                extract.set(Some(data));
                active_model.set(0);
                status.set(format!("生成成功，输出文件：{out}"));
            }
        }
        Ok(res) => {
            status.set(res.error.unwrap_or_else(|| "生成失败".into()));
        }
        Err(_) => {
            status.set("生成失败".into());
        }
    }
}

async fn start_bridge_server(mut status: Signal<String>) {
    status.set("正在启动 bridge-server...".into());
    let val = invoke("start_bridge_server", JsValue::UNDEFINED).await;
    match serde_wasm_bindgen::from_value::<CmdResult<String>>(val) {
        Ok(res) if res.ok => {
            status.set(res.data.unwrap_or_else(|| "bridge-server 已启动".into()));
        }
        Ok(res) => {
            status.set(res.error.unwrap_or_else(|| "启动失败".into()));
        }
        Err(_) => {
            status.set("启动失败".into());
        }
    }
}

async fn run_main_js(mut status: Signal<String>) {
    status.set("正在运行 run_main.js...".into());
    let val = invoke("run_main_js", JsValue::UNDEFINED).await;
    match serde_wasm_bindgen::from_value::<CmdResult<String>>(val) {
        Ok(res) if res.ok => {
            status.set(res.data.unwrap_or_else(|| "run_main.js 已启动".into()));
        }
        Ok(res) => {
            status.set(res.error.unwrap_or_else(|| "运行失败".into()));
        }
        Err(_) => {
            status.set("运行失败".into());
        }
    }
}

/// 从 Tauri 拖放事件中取出文件路径列表。
/// 事件参数为 `{ event, id, payload }`，payload 形如 `{ paths: [...], position: {...} }`。
fn paths_from_drop_event(event: &JsValue) -> Vec<String> {
    if event.is_undefined() || event.is_null() {
        return Vec::new();
    }
    let obj = match event.dyn_ref::<js_sys::Object>() {
        Some(o) => o,
        None => return Vec::new(),
    };
    let payload = match Reflect::get(obj, &JsValue::from_str("payload")) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if payload.is_undefined() || payload.is_null() {
        return Vec::new();
    }
    let paths = match Reflect::get(&payload, &JsValue::from_str("paths")) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if paths.is_undefined() || paths.is_null() {
        return Vec::new();
    }
    Array::from(&paths)
        .iter()
        .filter_map(|v| v.as_string())
        .collect()
}

/// 依据文件名推断该拖入文件应进入哪个虚线框（0=引脚定义，1=完整BOM）。
fn infer_zone(files: &[String], pin_path: &Signal<String>) -> u8 {
    let name = files[0].to_lowercase();
    if name.contains("引脚") || name.contains("定义") {
        0
    } else if name.contains("bom"){
        1
    } else if pin_path.read().is_empty() {
        0
    } else {
        1
    }
}

fn setup_file_drop(
    mut pin_path: Signal<String>,
    mut bom_path: Signal<String>,
    pin_preview: Signal<Option<XlsxPreview>>,
    bom_preview: Signal<Option<XlsxPreview>>,
    mut hover_zone: Signal<Option<u8>>,
    mut extract: Signal<Option<ExtractResult>>,
    status: Signal<String>,
) {
    // 拖入窗口：根据文件名高亮对应的虚线框
    let on_enter = Closure::wrap(Box::new(move |event: JsValue| {
        let files: Vec<String> = paths_from_drop_event(&event)
            .into_iter()
            .filter(|p| p.to_lowercase().ends_with(".xlsx"))
            .collect();
        if !files.is_empty() {
            hover_zone.set(Some(infer_zone(&files, &pin_path)));
        }
    }) as Box<dyn FnMut(JsValue)>);
    let f_enter: js_sys::Function = on_enter.as_ref().clone().into();

    // 松手：加载文件到对应虚线框
    let on_drop = Closure::wrap(Box::new(move |event: JsValue| {
        let mut files: Vec<String> = paths_from_drop_event(&event)
            .into_iter()
            .filter(|p| p.to_lowercase().ends_with(".xlsx"))
            .collect();
        if files.is_empty() {
            return;
        }
        let zone = {
            let hovered = *hover_zone.read();
            hovered.unwrap_or_else(|| infer_zone(&files, &pin_path))
        };
        hover_zone.set(None);
        let file = files.remove(0);
        extract.set(None);
        if zone == 0 {
            pin_path.set(file.clone());
            wasm_bindgen_futures::spawn_local(load_preview(file, pin_preview, status));
        } else {
            bom_path.set(file.clone());
            wasm_bindgen_futures::spawn_local(load_preview(file, bom_preview, status));
        }
    }) as Box<dyn FnMut(JsValue)>);
    let f_drop: js_sys::Function = on_drop.as_ref().clone().into();

    // 拖离窗口 / 松开取消：取消悬停高亮
    let on_leave = Closure::wrap(Box::new(move |_: JsValue| {
        hover_zone.set(None);
    }) as Box<dyn FnMut(JsValue)>);
    let f_leave: js_sys::Function = on_leave.as_ref().clone().into();

    wasm_bindgen_futures::spawn_local(async move {
        let _ = listen("tauri://drag-enter", f_enter).await;
        let _ = listen("tauri://drag-drop", f_drop).await;
        let _ = listen("tauri://drag-leave", f_leave).await;
    });

    on_enter.forget();
    on_drop.forget();
    on_leave.forget();
}

fn render_output_rows(rows: &[Vec<String>]) -> Element {
    rsx! {
        for row in rows {
            tr {
                for c in row {
                    td { "{c}" }
                }
            }
        }
    }
}

fn render_tabs(models: &[ModelResult], active: usize, mut active_model: Signal<usize>) -> Element {
    rsx! {
        for (i , m) in models.iter().enumerate() {
            button {
                class: if i == active { "tab tab-active" } else { "tab" },
                onclick: move |_| active_model.set(i),
                "{m.name}"
            }
        }
    }
}

pub fn App() -> Element {
    let pin_path = use_signal(|| String::new());
    let bom_path = use_signal(|| String::new());
    let pin_preview = use_signal(|| Option::<XlsxPreview>::None);
    let bom_preview = use_signal(|| Option::<XlsxPreview>::None);
    let extract = use_signal(|| Option::<ExtractResult>::None);
    let active_model = use_signal(|| 0usize);
    let mut hover_zone = use_signal(|| Option::<u8>::None);
    let busy = use_signal(|| false);
    let status = use_signal(|| String::new());
    let status_bridge_server = use_signal(|| String::new());
    let status_module_extract = use_signal(|| String::new());

    use_effect(move || {
        setup_file_drop(
            pin_path,
            bom_path,
            pin_preview,
            bom_preview,
            hover_zone,
            extract,
            status,
        );
    });

    let pin_path_val = pin_path.read().clone();
    let bom_path_val = bom_path.read().clone();
    let pin_preview_val = pin_preview.read().clone();
    let bom_preview_val = bom_preview.read().clone();
    let extract_val = extract.read().clone();
    let active = active_model.read().clone();
    let hover = hover_zone.read().clone();
    let busy_val = busy.read().clone();
    let status_val = status.read().clone();
    let status_bridge_sever_val = status_bridge_server.read().clone();
    let status_module_extract_val = status_module_extract.read().clone();
    

    rsx! {
        link { rel: "stylesheet", href: CSS }
        main { class: "container",
            h1 { class: "app-title", "BOM 提取工具" }

            div { class: "upload-row",
                div {
                    class: format!("upload-zone{}", if hover == Some(0) { " dragover" } else { "" }),
                    onclick: move |_| {
                        wasm_bindgen_futures::spawn_local(
                            pick_file(0, pin_path, bom_path, pin_preview, bom_preview, extract, status),
                        );
                    },
                    ondragenter: move |_| hover_zone.set(Some(0)),
                    ondragover: move |ev| ev.prevent_default(),
                    ondragleave: move |_| hover_zone.set(None),
                    ondrop: move |ev| ev.prevent_default(),
                    svg {
                        class: "zone-icon",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        path { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }
                        polyline { points: "14 2 14 8 20 8" }
                        line {
                            x1: "8",
                            y1: "13",
                            x2: "16",
                            y2: "13",
                        }
                        line {
                            x1: "8",
                            y1: "17",
                            x2: "16",
                            y2: "17",
                        }
                    }
                    div { class: "zone-title", "引脚定义" }
                    div { class: "zone-desc", "请拖入或选择引脚定义文件" }
                    button {
                        class: "file-btn",
                        onclick: move |ev| {
                            ev.stop_propagation();
                            wasm_bindgen_futures::spawn_local(
                                pick_file(0, pin_path, bom_path, pin_preview, bom_preview, extract, status),
                            );
                        },
                        "选择文件"
                    }
                }
                div {
                    class: format!("upload-zone{}", if hover == Some(1) { " dragover" } else { "" }),
                    onclick: move |_| {
                        wasm_bindgen_futures::spawn_local(
                            pick_file(1, pin_path, bom_path, pin_preview, bom_preview, extract, status),
                        );
                    },
                    ondragenter: move |_| hover_zone.set(Some(1)),
                    ondragover: move |ev| ev.prevent_default(),
                    ondragleave: move |_| hover_zone.set(None),
                    ondrop: move |ev| ev.prevent_default(),
                    svg {
                        class: "zone-icon",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        path { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }
                        polyline { points: "14 2 14 8 20 8" }
                        line {
                            x1: "8",
                            y1: "13",
                            x2: "16",
                            y2: "13",
                        }
                        line {
                            x1: "8",
                            y1: "17",
                            x2: "16",
                            y2: "17",
                        }
                    }
                    div { class: "zone-title", "完整BOM" }
                    div { class: "zone-desc", "请拖入或选择完整BOM文件" }
                    button {
                        class: "file-btn",
                        onclick: move |ev| {
                            ev.stop_propagation();
                            wasm_bindgen_futures::spawn_local(
                                pick_file(1, pin_path, bom_path, pin_preview, bom_preview, extract, status),
                            );
                        },
                        "选择文件"
                    }
                }
            }

            div { class: "path-row",
                div { class: "path-item",
                    span { class: "path-label", "引脚定义路径" }
                    span { class: "path-value", "{pin_path_val}" }
                }
                div { class: "path-item",
                    span { class: "path-label", "完整BOM路径" }
                    span { class: "path-value", "{bom_path_val}" }
                }
            }

            div { class: "section-title", "文件预览" }
            div { class: "preview-row",
                div { class: "preview-panel",
                    div { class: "preview-head", "引脚定义" }
                    if let Some(pin) = &pin_preview_val {
                        div { class: "table-wrap",
                            table {
                                thead {
                                    tr {
                                        for h in &pin.headers {
                                            th { "{h}" }
                                        }
                                    }
                                }
                                tbody { {render_output_rows(&pin.rows)} }
                            }
                        }
                    } else {
                        div { class: "preview-empty", "请拖入或选择引脚定义文件" }
                    }
                }
                div { class: "preview-panel",
                    div { class: "preview-head", "完整BOM" }
                    if let Some(bom) = &bom_preview_val {
                        div { class: "table-wrap",
                            table {
                                thead {
                                    tr {
                                        for h in &bom.headers {
                                            th { "{h}" }
                                        }
                                    }
                                }
                                tbody { {render_output_rows(&bom.rows)} }
                            }
                        }
                    } else {
                        div { class: "preview-empty", "请拖入或选择完整BOM文件" }
                    }
                }
            }
            div { class: "actions_item",
                div { class: "actions",
                    button {
                        class: "extract-btn",
                        disabled: busy_val,
                        onclick: move |_| {
                            wasm_bindgen_futures::spawn_local(start_bridge_server(status_bridge_server));
                        },
                        "启动bridge-server"
                    }
                    if !status_bridge_sever_val.is_empty() {
                        div { class: "status", "{status_bridge_sever_val}" }
                    }
                }
                div { class: "actions",
                    button {
                        class: "extract-btn",
                        disabled: busy_val,
                        onclick: move |_| {
                            wasm_bindgen_futures::spawn_local(run_main_js(status_module_extract));
                        },
                        "开始提取模块位号"
                    }
                    if !status_module_extract_val.is_empty() {
                        div { class: "status", "{status_module_extract_val}" }
                    }
                }
                div { class: "actions",
                    button {
                        class: "extract-btn",
                        disabled: busy_val,
                        onclick: move |_| {
                            wasm_bindgen_futures::spawn_local(
                                run_extract(pin_path, bom_path, extract, active_model, busy, status),
                            );
                        },
                        "开始输出选贴BOM"
                    }
                    if !status_val.is_empty() {
                        div { class: "status", "{status_val}" }
                    }
                }
            }

            div { class: "section-title", "选贴BOM输出预览" }
            div { class: "output-panel",
                if let Some(er) = &extract_val {
                    if !er.models.is_empty() {
                        div { class: "tabs", {render_tabs(&er.models, active, active_model)} }
                        if let Some(m) = er.models.get(active) {
                            div { class: "output-table-title", "选贴BOM" }
                            div { class: "table-wrap output-table-wrap",
                                table {
                                    thead {
                                        tr {
                                            for h in &m.kept_rows[0] {
                                                th { "{h}" }
                                            }
                                        }
                                    }
                                    tbody { {render_output_rows(&m.kept_rows[1..])} }
                                }
                            }
                            div { class: "output-table-title", "选贴差异BOM" }
                            div { class: "table-wrap output-table-wrap",
                                table {
                                    thead {
                                        tr {
                                            for h in &m.diff_rows[0] {
                                                th { "{h}" }
                                            }
                                        }
                                    }
                                    tbody { {render_output_rows(&m.diff_rows[1..])} }
                                }
                            }
                        }
                    } else {
                        div { class: "preview-empty",
                            "请先点击「开始输出选贴BOM」生成结果"
                        }
                    }
                } else {
                    div { class: "preview-empty", "请先点击「开始输出选贴BOM」生成结果" }
                }
            }
        }
    }
}

