use calamine::{open_workbook, Data, Reader, Xlsx};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Kept BOM rows (including the header row) for this model.
    pub kept_rows: Vec<Vec<String>>,
    /// Designators deleted from the full BOM for this model.
    pub deleted: Vec<String>,
    /// Designators kept in the full BOM for this model.
    pub kept: Vec<String>,
}

/// A designator -> related pin-definition row indices group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedGroup {
    pub designator: String,
    pub rows: Vec<usize>,
}

/// Result of the extraction run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractResult {
    pub models: Vec<ModelResult>,
    /// For every designator found in the full BOM, the pin-definition rows that mention it.
    pub related_groups: Vec<RelatedGroup>,
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
        .unwrap_or_else(|| {
            if bom_preview.headers.len() > 1 {
                1
            } else {
                0
            }
        });

    // designator -> indices of pin-definition rows that mention it (in the module column)
    let mut related: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, (_, modules, _)) in pin.entries.iter().enumerate() {
        for m in modules {
            related.entry(m.clone()).or_default().push(i);
        }
    }

    // per model: (deleted designators, kept designators)
    let mut model_names = Vec::new();
    let mut per_model_deleted: Vec<Vec<String>> = Vec::new();
    let mut per_model_kept: Vec<Vec<String>> = Vec::new();

    for (model_idx, (_, model_name)) in pin.model_cols.iter().enumerate() {
        let mut deleted = Vec::new();
        let mut kept = Vec::new();
        for (designator, indices) in related.iter() {
            if indices.is_empty() {
                kept.push(designator.clone());
                continue;
            }
            let all_unused = indices
                .iter()
                .all(|&i| !pin.entries[i].2.get(model_idx).copied().unwrap_or(false));
            if all_unused {
                deleted.push(designator.clone());
            } else {
                kept.push(designator.clone());
            }
        }
        model_names.push(model_name.clone());
        per_model_deleted.push(deleted);
        per_model_kept.push(kept);
    }

    // Build output BOM rows per model (header + kept rows)
    let mut models = Vec::new();
    for (model_idx, name) in model_names.iter().enumerate() {
        let deleted: std::collections::HashSet<&String> =
            per_model_deleted[model_idx].iter().collect();
        let kept_rows: Vec<Vec<String>> = bom_preview
            .rows
            .iter()
            .filter(|row| {
                let d = row.get(bom_designator_col).cloned().unwrap_or_default();
                !d.is_empty() && !deleted.contains(&d)
            })
            .cloned()
            .collect();
        let mut full = Vec::with_capacity(kept_rows.len() + 1);
        full.push(bom_preview.headers.clone());
        full.extend(kept_rows);
        models.push(ModelResult {
            name: name.clone(),
            kept_rows: full,
            deleted: per_model_deleted[model_idx].clone(),
            kept: per_model_kept[model_idx].clone(),
        });
    }

    let output_path = build_output_path(bom_path)?;
    write_output_xlsx(&output_path, &models)?;

    let mut related_groups: Vec<RelatedGroup> = Vec::new();
    for (designator, rows) in related.into_iter() {
        if !rows.is_empty() {
            related_groups.push(RelatedGroup {
                designator,
                rows,
            });
        }
    }

    Ok(ExtractResult {
        models,
        related_groups,
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

fn write_output_xlsx(path: &str, models: &[ModelResult]) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook, XlsxAlign, XlsxBorder, XlsxColor};

    let mut workbook = Workbook::new(path);
    let header_format = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_align(XlsxAlign::Center)
        .set_border(XlsxBorder::Thin)
        .set_background_color(XlsxColor::RGB(0xD9E1F2));

    for model in models {
        let ws = workbook
            .add_worksheet()
            .set_name(&model.name)
            .map_err(|e| e.to_string())?;
        for (r, row) in model.kept_rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                if r == 0 {
                    ws.write_string(r as u32, c as u16, v, &header_format)
                        .map_err(|e| e.to_string())?;
                } else {
                    ws.write_string_only(r as u32, c as u16, v)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    workbook
        .close()
        .map_err(|e| format!("保存输出文件失败: {e}"))?;
    Ok(())
}
