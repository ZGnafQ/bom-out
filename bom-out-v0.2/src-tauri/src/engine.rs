use calamine::{Data, Reader, Xlsx, open_workbook};
use rust_xlsxwriter::{Format, Workbook, XlsxAlign, XlsxBorder, XlsxColor};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap};
//use serde_json::Value;
use std::path::{Path, PathBuf};
use std::string::String;
/// Preview of an xlsx file: the first sheet rendered as strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XlsxPreview {
    pub sheet: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Per-model output of the selected-paste BOM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResult {
    pub name: String,
    /// 选贴BOM rows (header + data rows), same columns as the input BOM.
    pub kept_rows: Vec<Vec<String>>,
    /// 差异BOM rows (header + data rows), input columns plus a 备注 column.
    pub diff_rows: Vec<Vec<String>>,
    /// Designators removed from the full BOM for this model.
    pub deleted: Vec<String>,
    /// Designators kept in the full BOM for this model.
    pub kept: Vec<String>,
}

/// Result of the extraction run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractResult {
    pub models: Vec<ModelResult>,
    /// Column index (0-based) of the designator column in the full BOM.
    pub bom_designator_col: usize,
    /// Absolute path of the generated xlsx file.
    pub output_path: String,
}

fn data_to_string(d: &Data) -> String {
    match d {
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => i.to_string(),
        Data::DateTime(_) => String::new(),
        Data::DateTimeIso(_) => String::new(),
        Data::DurationIso(_) => String::new(),
        Data::Bool(b) => b.to_string(),
        Data::Error(_) => String::new(),
        Data::Empty => String::new(),
    }
}

fn cell_is_used(cell: &str) -> bool {
    cell.contains('\u{25CF}')
}

fn split_modules(cell: &str) -> Vec<String> {
    cell.split(|c: char| c == ',' || c == '\u{FF0C}' || c == ';' || c == '\u{FF1B}')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract board name from file path
pub fn extract_board_name(file_path: &str) -> String {
    let file_name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    // 从文件名中提取板子名称
    // 例如："GP4.0F-引脚定义.xlsx" -> "gp4.0f"
    let base_name = file_name
        .split("引脚定义.xlsx")
        .next()
        .unwrap_or(file_name);
    
    // 转换小写并去除空格
    let board_name = base_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' /*|| *c == '-'*/)
        .collect::<String>()
        .to_lowercase();

    // 清理: 只保留字母、数字、点和连字符
    let cleaned: String = board_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' /*|| *c == '-'*/)
        .collect();

    cleaned.trim().to_string()
}

/// Read the first worksheet of an xlsx file as strings.
pub fn read_xlsx_preview(path: &str) -> Result<XlsxPreview, String> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).map_err(|e| format!("无法打开文件: {e}"))?;
    let Some((name, range)) = workbook.worksheets().into_iter().next() else {
        return Err("文件中没有工作表".into());
    };
    let mut it = range.rows();
    let headers: Vec<String> = match it.next() {
        Some(row) => row.iter().map(data_to_string).collect(),
        None => return Err("工作表为空".into()),
    };
    let rows: Vec<Vec<String>> = it.map(|row| row.iter().map(data_to_string).collect()).collect();
    let file_name_only = path.split('\\').last().unwrap_or(path);
    // 仅当文件名包含 "引脚定义" 时才保存 board_name.json
    if file_name_only.contains("引脚定义"){
        // 从文件路径中提取板子名称
        let board_name = extract_board_name(path);

        // 保存board_name 到 JSON 文件
        // let output_dir = std::path::Path::new(&path)
        //     .parent()
        //     .unwrap_or_else(|| std::path::Path::new("."));
        // let json_path = output_dir.join(format!("{}_board_name.json", board_name));
        // let json_path = output_dir.join(format!("board_name.json"));
        // 保存 board_name 到 JSON 文件
        std::fs::create_dir_all("d:/output").ok();
        let json_path = Path::new("d:/output").join("board_name.json");
        let json_data = serde_json::json!({
            "board_name": board_name
        });
        let _ = std::fs::write(
            &json_path,
            serde_json::to_string_pretty(&json_data).unwrap_or_else(|_| "{}".to_string())
        );
    }
    
    Ok(XlsxPreview {
        sheet: name,
        headers,
        rows,
    })
}

// json_map: 从 JSON 解析得到的HashMap<键, Vec<String>>
// pub json_map: String,
#[derive(Debug, Deserialize)]
pub struct JsonRoot{
    #[serde(rename = "ConnectorModule_total")]
    connector_module_total: HashMap<String, Vec<String>>,
}
// "./output/gp4.0f.json"
pub fn parse_json_module(json_path: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let json_str = std::fs::read_to_string(json_path).unwrap_or_default();
    let json_root: JsonRoot = serde_json::from_str(&json_str).unwrap();
    Ok(json_root.connector_module_total)
}
pub fn process_excel(pin_path: &str, json_path: &str) -> Result<(), String> {
    let output_pin_path = build_pin_output_path(pin_path)?;
    let pin_preview: XlsxPreview = read_xlsx_preview(pin_path)?;
    let mut new_workbook = Workbook::new(&output_pin_path);
    let new_ws = new_workbook.add_worksheet();
    let json_map = parse_json_module(json_path);
    let map = json_map.unwrap();
    let mut new_headers = pin_preview.headers.clone();
    new_headers.push("元件位号".to_string());
    // println!("new_headers: {:?}", new_headers); // 应显示末尾有"元件位号"
    // let wrap_format = Format::new().set_text_wrap();

    let header_format = Format::new()
        .set_bold()
        .set_font_size(10)
        .set_align(XlsxAlign::Center)
        .set_align(XlsxAlign::VerticalCenter)
        .set_border(XlsxBorder::Thin)
        .set_font_name("微软雅黑")
        .set_background_color(XlsxColor::RGB(0x08B3F1))
        // .set_background_color(XlsxColor::RGB(0x57CBE2))
        .set_text_wrap();

    let cell_format = Format::new()
        // .set_bold()
        .set_font_size(9)
        .set_align(XlsxAlign::Center)
        .set_align(XlsxAlign::VerticalCenter)
        .set_border(XlsxBorder::Thin)
        .set_font_name("微软雅黑")
        .set_text_wrap();
        // .set_background_color(XlsxColor::RGB(0x08B3F1))

    let connector_col = pin_preview.headers
        .iter()
        .enumerate()
        .filter(|_, h| h.contains("引脚号"))
        .last()
        .map(|idx, _| idx)
        .unwrap_or(0);
    new_ws.set_row_height(0, 70)
        .map_err(|e| e.to_string())?;   // 第一行设为 70 字符单位
    for (col, title) in new_headers.iter().enumerate() {
        new_ws.write_string(0, col as u16, title, &header_format)
            .map_err(|e| e.to_string())?;
        new_ws.set_column_width(col as u16, 7.5)
            .map_err(|e| e.to_string())?;
    }
    new_ws.set_column_width(6, 12)
            .map_err(|e| e.to_string())?;
    new_ws.set_column_width(7, 22)
            .map_err(|e| e.to_string())?;
    new_ws.set_column_width(8, 12)
            .map_err(|e| e.to_string())?;
    new_ws.set_column_width(9, 10)
            .map_err(|e| e.to_string())?;
    let last_column: usize = new_headers.clone().len() - 1;
    new_ws.set_column_width(last_column as u16, 9)
            .map_err(|e| e.to_string())?;
    for (r, row) in pin_preview.rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate(){
            new_ws.write_string((r+1) as u32, c as u16, v, &cell_format)
            .map_err(|e| e.to_string())?;
        }
    }
    for (r, row) in pin_preview.rows.iter().enumerate() {
        let new_col = pin_preview.headers.len() as u16;
        let connector = row.get(connector_col).cloned().unwrap_or_default();
        let array = map.get(&connector).cloned().unwrap_or_default();
        let new_val = array.join(",");
        new_ws.write_string((r+1) as u32, new_col, &new_val, &cell_format)
            .map_err(|e| e.to_string())?;
        new_ws.set_row_height((r+1) as u32, 30)
            .map_err(|e| e.to_string())?;
    }
    new_workbook
        .close()
        .map_err(|e| format!("保存输出文件失败: {e}"))?;
    OK(())
}

pub fn excel_result_test(extract_path: &str, module_path: &str) -> Result<(),String> {
    let output_result_path = build_output_path_you_want(extract_path, "差异分析")?;
    let extract_preview: XlsxPreview = read_xlsx_preview(extract_path)?;
    let module_preview: XlsxPreview = read_xlsx_preview(module_path)?;
    let mut new_workbook = Workbook::new(&output_result_path);
    let new_ws = new_workbook.add_worksheet();
    let mut new_headers = extract_preview.headers.clone();
    new_headers.push("核验结果".to_string());
    new_headers.push("相同位号".to_string());
    new_headers.push("差异位号".to_string());

    let header_format = Format::new()
        .set_bold()
        .set_font_size(10)
        .set_align(XlsxAlign::Center)
        .set_align(XlsxAlign::VerticalCenter)
        .set_border(XlsxBorder::Thin)
        .set_font_name("微软雅黑")
        .set_background_color(XlsxColor::RGB(0x08B3F1))
        // .set_background_color(XlsxColor::RGB(0x57CBE2))
        .set_text_wrap();

    let cell_format = Format::new()
        // .set_bold()
        .set_font_size(9)
        .set_align(XlsxAlign::Center)
        .set_align(XlsxAlign::VerticalCenter)
        .set_border(XlsxBorder::Thin)
        .set_font_name("微软雅黑")
        .set_text_wrap();
    
    let designator_col = extract_preview.headers
        .iter()
        .enumerate()
        .filter(|_, h| h.contains("元件位号"))
        .last()
        .map(|idx, _| idx)
        .unwrap_or(0);

    for (col, title) in new_headers.iter().enumerate() {
        new_ws.write_string(0, col as u16, title, &header_format)
            .map_err(|e| e.to_string())?;
        new_ws.set_column_width(col as u16, 7.5)
            .map_err(|e| e.to_string())?;
    }
    for (r, row) in extract_preview.rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            new_ws.write_string((r+1) as u32, c as u16, v, &cell_format)
            .map_err(|e| e.to_string())?;
        }
    }
    for (r, (row1, row2)) in extract_preview.rows.iter().zip(module_preview.rows.iter()).enumerate() {
        let cell_extract = row1.get(designator_col).cloned().unwrap_or_default();
        let cell_module = row2.get(designator_col).cloned().unwrap_or_default();
        let designator_extract = split_modules(&cell_extract);
        let designator_module = split_modules(&cell_module);
        if designator_extract.is_empty() {
            continue;
        }
        let mut designator_kept = Vec::new();
        let mut designator_diff = Vec::new();
        // let mut kept_rows = Vec::new();

        for designator in &designator_extract {
            if designator_module.contains(designator) {
                designator_kept.push(designator.to_owned().clone());
            }
            else {
                designator_diff.push(designator.to_owned().clone());
            }
        }
        for designators in designator_module {
            if designator_kept.contains(&designators) {
                continue;
            }
            if !designator_extract.contains(&designators) {
                designator_diff.push(designators.clone());
            }  
        }
        let new_col = extract_preview.headers.len() as u16;
        let same_val = designator_kept.join(",");
        let diff_val = designator_diff.join(",");
        let check_val = if designator_diff.is_empty() {
            "核验通过"
        }
        else {
            "核验不通过"
        };
        new_ws.write_string((r+1) as u32, new_col, check_val, &cell_format)
            .map_err(|e| e.to_string())?;
        new_ws.write_string((r+1) as u32, (new_col+1) as u16, &same_val, &cell_format)
            .map_err(|e| e.to_string())?;
        new_ws.write_string((r+1) as u32, (new_col+2) as u16, &diff_val, &cell_format)
            .map_err(|e| e.to_string())?;
        new_ws.set_row_height((r+1) as u32, 30)
            .map_err(|e| e.to_string())?;
    }
    new_workbook
        .close()
        .map_err(|e| format!("保存输出文件失败: {e}"))?;
    OK(())
}

#[derive(Debug, Deserialize)]
struct PinDef {
    #[allow(dead_code)]
    connector_col: usize,
    #[allow(dead_code)]
    module_col: usize,
    model_cols: Vec<(usize, String)>,
    /// rows: (connector, modules, per-model used flags)
    entries: Vec<(String, Vec<String>, Vec<bool>)>,
}

fn parse_pin_definition(headers: &[String], rows: &[Vec<String>]) -> PinDef {
    let connector_col = headers
        .iter()
        .enumerate()
        .filter(|(_, h)| h.contains("引脚号"))
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    // Find model columns (columns containing "型")
    let mut model_cols = Vec::new();
    for (idx, h) in headers.iter().enumerate() {
        if h.contains("型") && !h.contains("类型") {
            model_cols.push((idx, h.trim().to_string()));
        }
    }

    let module_col = headers
        .iter()
        .position(|h| h.contains("元件") || h.contains("位号"))
        .unwrap_or_else(|| headers.len().saturating_sub(1));
    
    let mut entries = Vec::new();
    for row in rows {
        let connector = row.get(connector_col).cloned().unwrap_or_default();
        let module_cell = row.get(module_col).cloned().unwrap_or_default();
        let modules = split_modules(&module_cell);
        let flags: Vec<bool> = model_cols
            .iter()
            .map(|(idx, _)| {
                let cell = row.get(*idx).cloned().unwrap_or_default();
                cell_is_used(&cell)
            })
            .collect();
        if connector.is_empty() {
            continue;
        }
        entries.push((connector, modules, flags));
    }
    
    PinDef {
        connector_col,
        module_col,
        model_cols,
        entries,
    }
}

/// Extract the selected-paste BOM.
///
/// The pin-definition file decides, per model (车型), which designators (位号) are used.
/// A designator is used by a model when it appears in the 所属模块位号 of a pin-definition
/// row whose marker for that model is the black solid circle (●). Designators that are
/// mentioned somewhere in the pin definition but not used by a model are treated as
/// removed for that model; designators that are never mentioned are kept (same as v1).
///
/// The full BOM is grouped by 物料编码 and every row holds all its designators inside the
/// 位号 cell. The 选贴BOM keeps the input columns but filters the 位号 cell down to the
/// designators used by the model; the 差异BOM keeps the removed designators and adds a
/// 备注 column filled with "删除". Rows whose filtered 位号 would be empty are dropped.
pub fn extract_selected_bom(pin_path: &str, bom_path: &str) -> Result<ExtractResult, String> {
    let board_name = extract_board_name(&pin_path);
    let json_path = format!("./output/{}.json", board_name);
    process_excel(pin_path, &json_path)?;
    let p = Path::new(pin_path);
    let parent = p
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = p
        .file_stem()
        .map(||)
    let pin_preview = read_xlsx_preview(pin_path)?;
    let bom_preview = read_xlsx_preview(bom_path)?;

    if pin_preview.headers.is_empty() || bom_preview.headers.is_empty() {
        return Err("引脚定义或完整BOM文件为空".into());
    }

    let pin = parse_pin_definition(&pin_preview.headers, &pin_preview.rows);

    if pin.model_cols.is_empty() {
        return Err("引脚定义中未检测到车型列（如 车型A/车型B/车型C）".into());
    }

    let bom_designator_col = bom_preview
        .headers
        .iter()
        .position(|h| h.contains("位号"))
        .unwrap_or_else(|| bom_preview.headers.len().saturating_sub(1));

    // Per model: the set of designators used by that model, plus the union of all
    // designators mentioned anywhere in the pin definition.
    let mut used_per_model: Vec<HashSet<String>> = vec![HashSet::new(); pin.model_cols.len()];
    let mut mentioned: HashSet<String> = HashSet::new();
    for (_, modules, flags) in &pin.entries {
        for m in modules {
            mentioned.insert(m.clone());
        }
        for (mi, flag) in flags.iter().enumerate() {
            if *flag {
                for m in modules {
                    used_per_model[mi].insert(m.clone());
                }
            }
        }
    }

    let mut diff_header = bom_preview.headers.clone();
    if !diff_header.iter().any(|h| h.contains("备注")) {
        diff_header.push("备注".into());
    }

    let mut models = Vec::new();
    for (model_idx, (_, model_name)) in pin.model_cols.iter().enumerate() {
        let used = &used_per_model[model_idx];
        let mut kept_rows: Vec<Vec<String>> = Vec::new();
        let mut diff_rows: Vec<Vec<String>> = Vec::new();
        let mut deleted = Vec::new();
        let mut kept = Vec::new();

        for row in &bom_preview.rows {
            let cell = row.get(bom_designator_col).cloned().unwrap_or_default();
            let designators = split_modules(&cell);
            if designators.is_empty() {
                continue;
            }
            let mut row_kept = Vec::new();
            let mut row_removed = Vec::new();
            for d in designators {
                if used.contains(&d) || !mentioned.contains(&d) {
                    row_kept.push(d.clone());
                    kept.push(d.clone());
                } else {
                    row_removed.push(d.clone());
                    deleted.push(d.clone());
                }
            }

            if !row_kept.is_empty() {
                let mut r = row.clone();
                if let Some(cell) = r.get_mut(bom_designator_col) {
                    *cell = row_kept.join(",");
                }
                kept_rows.push(r);
            }
            if !row_removed.is_empty() {
                let mut r = row.clone();
                if let Some(cell) = r.get_mut(bom_designator_col) {
                    *cell = row_removed.join(",");
                }
                r.push("删除".into());
                diff_rows.push(r);
            }
        }

        let mut full_kept = Vec::with_capacity(kept_rows.len() + 1);
        full_kept.push(bom_preview.headers.clone());
        full_kept.extend(kept_rows);

        let mut full_diff = Vec::with_capacity(diff_rows.len() + 1);
        full_diff.push(diff_header.clone());
        full_diff.extend(diff_rows);

        models.push(ModelResult {
            name: model_name.clone(),
            kept_rows: full_kept,
            diff_rows: full_diff,
            deleted,
            kept,
        });
    }

    let output_path = build_output_path(bom_path)?;
    write_output_xlsx(&output_path, &models)?;

    Ok(ExtractResult {
        models,
        bom_designator_col,
        output_path,
    })
}

fn build_output_path(bom_path: &str) -> Result<String, String> {
    let p = Path::new(bom_path);
    let parent = p
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bom".into());
    let mut out = parent.join(format!("{stem}_选贴bom.xlsx"));
    let mut counter = 1u32;
    while out.exists() {
        out = parent.join(format!("{stem}_选贴bom_{counter}.xlsx"));
        counter += 1;
    }
    Ok(out.to_string_lossy().into_owned())
}

fn safe_sheet_name(s: &str) -> String {
    let mut out: String = s.chars().take(31).collect();
    for bad in ['\\', '/', '*', '?', ':', '[', ']'] {
        out = out.replace(bad, "_");
    }
    out
}

fn write_sheet(
    workbook: &mut Workbook,
    name: &str,
    rows: &[Vec<String>],
    header_format: &Format,
) -> Result<(), String> {
    let ws = workbook
        .add_worksheet()
        .set_name(&safe_sheet_name(name))
        .map_err(|e| e.to_string())?;
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            if r == 0 {
                ws.write_string(r as u32, c as u16, v, header_format)
                    .map_err(|e| e.to_string())?;
            } else {
                ws.write_string_only(r as u32, c as u16, v)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn write_output_xlsx(path: &str, models: &[ModelResult]) -> Result<(), String> {
    let mut workbook = Workbook::new(path);
    let header_format = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_align(XlsxAlign::Center)
        .set_border(XlsxBorder::Thin)
        .set_background_color(XlsxColor::RGB(0xD9E1F2));

    for model in models {
        write_sheet(
            &mut workbook,
            &format!("{}选贴BOM", model.name),
            &model.kept_rows,
            &header_format,
        )?;
        write_sheet(
            &mut workbook,
            &format!("{}差异BOM", model.name),
            &model.diff_rows,
            &header_format,
        )?;
    }

    workbook
        .close()
        .map_err(|e| format!("保存输出文件失败: {e}"))?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> (std::path::PathBuf, String, String) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let pin = dir.join("GP4.0F-引脚定义.xlsx");
        let bom = dir.join("GP4.0F-完整bom.xlsx");
        (dir, pin.to_string_lossy().into_owned(), bom.to_string_lossy().into_owned())
    }

    fn clean_outputs(dir: &std::path::Path) {
        let prefix = dir.join("GP4.0F-完整bom_选贴bom");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.to_string_lossy().starts_with(prefix.to_string_lossy().as_ref()) {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
    }

    #[test]
    fn extract_sample_matches_expected() {
        let (dir, pin, bom) = sample_files();
        let result = extract_selected_bom(&pin, &bom).expect("extraction should succeed");

        assert_eq!(result.models.len(), 3);

        // 车型A
        let a = &result.models[0];
        assert_eq!(a.name, "车型A");
        assert_eq!(a.kept_rows[0], vec!["序号", "comment", "物料编码", "位号"]);
        assert_eq!(a.kept_rows[1], vec!["1", "cyt4bf", "1111-1", "U12_1"]);
        assert_eq!(a.kept_rows[2], vec!["2", "100nF", "222-3", "C12,C12_2"]);
        assert_eq!(a.kept_rows[3], vec!["3", "100kΩ", "444-5", "R12,R12_1"]);
        assert_eq!(a.kept_rows[4], vec!["4", "10kΩ", "123-4", "R13"]);
        assert_eq!(a.kept_rows.len(), 5);
        assert_eq!(a.diff_rows[0], vec!["序号", "comment", "物料编码", "位号", "备注"]);
        assert_eq!(
            a.diff_rows[1],
            vec!["1", "cyt4bf", "1111-1", "U12,U12_2,U12_3", "删除"]
        );
        assert_eq!(a.diff_rows.len(), 2);

        // 车型B
        let b = &result.models[1];
        assert_eq!(b.name, "车型B");
        assert_eq!(b.kept_rows[1], vec!["1", "cyt4bf", "1111-1", "U12,U12_1,U12_2,U12_3"]);
        assert_eq!(b.kept_rows[2], vec!["2", "100nF", "222-3", "C12"]);
        assert_eq!(b.kept_rows[3], vec!["3", "100kΩ", "444-5", "R12,R12_1"]);
        assert_eq!(b.kept_rows.len(), 4);
        assert_eq!(b.diff_rows[1], vec!["2", "100nF", "222-3", "C12_2", "删除"]);
        assert_eq!(b.diff_rows[2], vec!["4", "10kΩ", "123-4", "R13", "删除"]);
        assert_eq!(b.diff_rows.len(), 3);

        // 车型C
        let c = &result.models[2];
        assert_eq!(c.name, "车型C");
        assert_eq!(c.kept_rows[1], vec!["1", "cyt4bf", "1111-1", "U12,U12_1,U12_2,U12_3"]);
        assert_eq!(c.kept_rows[2], vec!["2", "100nF", "222-3", "C12,C12_2"]);
        assert_eq!(c.kept_rows[3], vec!["4", "10kΩ", "123-4", "R13"]);
        assert_eq!(c.kept_rows.len(), 4);
        assert_eq!(c.diff_rows[1], vec!["3", "100kΩ", "444-5", "R12,R12_1", "删除"]);
        assert_eq!(c.diff_rows.len(), 2);

        clean_outputs(&dir);
    }
}
