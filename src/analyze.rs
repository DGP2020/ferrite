//! C semantic analysis and migration unit extraction (ADR 0002 & ADR 0007).

use crate::models::{FeatureVector, MigrationUnit, UnitStatus};
use std::path::Path;

pub struct Analyzer;

impl Analyzer {
    /// Walk a C source tree or file and return extracted MigrationUnits.
    pub fn analyze_dir(&self, root: &Path) -> anyhow::Result<Vec<MigrationUnit>> {
        let mut units = Vec::new();
        if root.is_file() {
            if let Some(ext) = root.extension() {
                if ext == "c" || ext == "h" {
                    units.extend(self.analyze_file(root)?);
                }
            }
        } else if root.is_dir() {
            for entry in walkdir(root)? {
                if let Some(ext) = entry.extension() {
                    if ext == "c" {
                        units.extend(self.analyze_file(&entry)?);
                    }
                }
            }
        }
        Ok(units)
    }

    /// Extract migration units from a single C source file.
    pub fn analyze_file(&self, path: &Path) -> anyhow::Result<Vec<MigrationUnit>> {
        let content = std::fs::read_to_string(path)?;
        let mut units = Vec::new();

        // For the vertical slice, parse standard C function definitions.
        // If clang is present, clang -Xclang -ast-dump=json can be used;
        // here we provide robust lexical extraction as base/fallback.
        let mut lines = content.lines().enumerate().peekable();
        while let Some((line_idx, line)) = lines.next() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("/*")
            {
                continue;
            }

            // Look for function header, e.g. "int clamp(int val, int min, int max)"
            if trimmed.contains('(')
                && (trimmed.ends_with('{')
                    || lines
                        .peek()
                        .is_some_and(|(_, next)| next.trim().starts_with('{')))
            {
                let sig_part = if trimmed.ends_with('{') {
                    trimmed.trim_end_matches('{').trim()
                } else {
                    trimmed
                };

                if let Some((fn_name, rust_contract)) = parse_c_signature(sig_part) {
                    // Collect body until closing brace
                    let mut body_lines = vec![line.to_string()];
                    let mut brace_depth = 0;
                    if line.contains('{') {
                        brace_depth += line.matches('{').count() as i32;
                    }
                    if line.contains('}') {
                        brace_depth -= line.matches('}').count() as i32;
                    }

                    while brace_depth > 0 || body_lines.len() == 1 {
                        if let Some((_, next_line)) = lines.next() {
                            if next_line.contains('{') {
                                brace_depth += next_line.matches('{').count() as i32;
                            }
                            if next_line.contains('}') {
                                brace_depth -= next_line.matches('}').count() as i32;
                            }
                            body_lines.push(next_line.to_string());
                            if brace_depth <= 0 && body_lines.len() > 1 {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    let body = body_lines.join("\n");
                    let features = Self::features(&body, 0);

                    units.push(MigrationUnit {
                        id: format!("fn_{}", fn_name),
                        name: fn_name,
                        signature: sig_part.to_string(),
                        rust_contract_sig: rust_contract,
                        source_ref: format!("{}:{}", path.display(), line_idx + 1),
                        source_body: body,
                        callees: vec![],
                        features,
                        status: UnitStatus::Analyzed,
                        assurance_level: None,
                    });
                }
            }
        }

        Ok(units)
    }

    /// Derive the feature vector the plan stage and harness generator consume.
    pub fn features(body: &str, callee_count: usize) -> FeatureVector {
        let line_count = body.lines().count() as u32;
        let has_pointers = body.contains('*');
        let has_strings = body.contains("char *") || body.contains("char*");
        let has_loops = body.contains("for") || body.contains("while") || body.contains("goto");

        let has_buffer_indicators = body.contains('[')
            || body.contains("len")
            || body.contains("size")
            || body.contains("count")
            || body.contains("buf")
            || body.contains("size_t");

        let has_out_params = has_pointers
            && (body.contains("out") || body.contains("*out") || body.contains("*res"));

        let harnessable = !has_pointers || has_strings || has_buffer_indicators;

        FeatureVector {
            ast_node_count: body.chars().count() as u32 / 4,
            line_count,
            has_loops,
            has_pointers,
            has_strings,
            has_out_params,
            harnessable,
            callee_count: callee_count as u32,
            ..Default::default()
        }
    }
}

fn walkdir(dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir(&path)?);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Helper that parses basic C signature and produces the idiomatic Rust signature contract.
pub fn parse_c_signature(sig: &str) -> Option<(String, String)> {
    let paren_start = sig.find('(')?;
    let paren_end = sig.rfind(')')?;

    let left = sig[..paren_start].trim();
    let fn_name = left.split_whitespace().last()?.trim_start_matches('*');

    let params_str = &sig[paren_start + 1..paren_end].trim();
    let mut rust_params: Vec<(String, String)> = Vec::new();

    if !params_str.is_empty() && *params_str != "void" {
        let raw_params: Vec<&str> = params_str.split(',').map(|s| s.trim()).collect();
        let mut i = 0;
        while i < raw_params.len() {
            let p = raw_params[i];
            let tokens: Vec<&str> = p.split_whitespace().collect();
            if tokens.len() >= 2 {
                let p_name = tokens.last().unwrap().trim_start_matches('*');
                let type_part = p[..p.rfind(p_name).unwrap_or(p.len())].trim();
                let is_pointer = p.contains('*');
                let is_const = type_part.contains("const");

                if is_pointer {
                    if type_part.contains("uint8_t") || type_part.contains("unsigned char") {
                        let slice_ty = if is_const { "&[u8]" } else { "&mut [u8]" };
                        rust_params.push((p_name.to_string(), slice_ty.to_string()));
                        // If next parameter is a companion length (`len`, `size`, `cap`, `n`), fold it into the slice
                        if i + 1 < raw_params.len() {
                            let next_p = raw_params[i + 1];
                            if !next_p.contains('*')
                                && (next_p.contains("size_t")
                                    || next_p.ends_with(" len")
                                    || next_p.ends_with(" cap")
                                    || next_p.ends_with(" size"))
                            {
                                i += 2;
                                continue;
                            }
                        }
                    } else if type_part.contains("char") {
                        rust_params.push((p_name.to_string(), "&str".to_string()));
                    } else {
                        // Reject unsafe pointer truncation to integer (CWE-197)
                        return None;
                    }
                } else {
                    let clean_type = type_part.replace("const", "").trim().to_string();
                    let rust_type = match clean_type.as_str() {
                        "int" | "int32_t" | "signed int" => "i32",
                        "unsigned" | "unsigned int" | "uint32_t" => "u32",
                        "int64_t" | "long" | "long long" => "i64",
                        "uint64_t" | "unsigned long" | "unsigned long long" => "u64",
                        "size_t" => "usize",
                        "uint8_t" | "unsigned char" => "u8",
                        "int8_t" | "signed char" => "i8",
                        "uint16_t" | "unsigned short" => "u16",
                        "int16_t" | "short" => "i16",
                        "float" => "f32",
                        "double" => "f64",
                        "bool" | "_Bool" => "bool",
                        _ => return None, // Do not silently coerce unknown types to i32
                    };
                    rust_params.push((p_name.to_string(), rust_type.to_string()));
                }
            }
            i += 1;
        }
    }

    let ret_type_c = left
        .rsplit_once(fn_name)?
        .0
        .replace("static", "")
        .replace("inline", "")
        .trim()
        .to_string();
    let rust_ret = if ret_type_c.contains('*') {
        return None; // Raw pointer returns wait for Phase 2
    } else {
        match ret_type_c.as_str() {
            "int" | "int32_t" | "signed int" => " -> i32",
            "unsigned" | "unsigned int" | "uint32_t" => " -> u32",
            "int64_t" | "long" | "long long" => " -> i64",
            "uint64_t" | "unsigned long" | "unsigned long long" => " -> u64",
            "size_t" => " -> usize",
            "uint8_t" | "unsigned char" => " -> u8",
            "uint16_t" | "unsigned short" => " -> u16",
            "int16_t" | "short" => " -> i16",
            "float" => " -> f32",
            "double" => " -> f64",
            "bool" | "_Bool" => " -> bool",
            "void" => "",
            _ => return None,
        }
    };

    let formatted_params: Vec<String> = rust_params
        .into_iter()
        .map(|(n, t)| format!("{}: {}", n, t))
        .collect();
    let rust_contract = format!(
        "pub fn {}({}){}",
        fn_name,
        formatted_params.join(", "),
        rust_ret
    );
    Some((fn_name.to_string(), rust_contract))
}
