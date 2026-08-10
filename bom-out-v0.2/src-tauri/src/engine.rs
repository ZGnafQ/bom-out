use calamine::{open_workbook, Data, Reader, Xlsx};
use rust_xlsxwriter::{Format, Workbook, XlsxAlign, XlsxBorder, XlsxColor};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
    Ok(XlsxPreview {
        sheet: name,
        headers,
        rows,
    })
}

struct PinDef {
    #[allow(dead_code)]
    connector_col: usize,
    #[allow(dead_code)]
    signal_col: Option<usize>,
    #[allow(dead_code)]
    module_col: usize,
    model_cols: Vec<(usize, String)>,
    /// rows: (connector, modules, per-model used flags)
    entries: Vec<(String, Vec<String>, Vec<bool>)>,
}

fn parse_pin_definition(headers: &[String], rows: &[Vec<String>]) -> PinDef {
    let connector_col = 0usize;
    let module_col = headers
        .iter()
        .position(|h| h.contains("所属") || h.contains("模块"))
        .unwrap_or_else(|| headers.len().saturating_sub(1));
    let signal_col = headers.iter().position(|h| h.contains("信号"));

    let mut model_cols = Vec::new();
    for (idx, h) in headers.iter().enumerate() {
        if idx == connector_col || idx == module_col || Some(idx) == signal_col {
            continue;
        }
        if h.trim().is_empty() {
            continue;
        }
        model_cols.push((idx, h.trim().to_string()));
    }

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
        signal_col,
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
