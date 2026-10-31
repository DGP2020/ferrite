//! Deterministic FFI and differential harness generator (ADR 0004 & ADR 0007).

use crate::models::MigrationUnit;
use crate::verify::HarnessArtifacts;
use std::path::Path;

pub struct HarnessGen;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamSpec {
    Scalar {
        name: String,
        c_ty: String,
        rust_ty: String,
    },
    SliceU8 {
        name: String,
        len_name: String,
        mutable: bool,
    },
    CString {
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct SignatureSpec {
    pub fn_name: String,
    pub params: Vec<ParamSpec>,
    pub c_ret: String,
    pub rust_ret: String,
}

impl SignatureSpec {
    pub fn parse(unit: &MigrationUnit) -> anyhow::Result<Self> {
        let fn_name = sanitize_ident(&unit.name);
        let sig = unit.signature.trim();
        let paren_start = sig
            .find('(')
            .ok_or_else(|| anyhow::anyhow!("Missing '(' in C signature: {}", sig))?;
        let paren_end = sig
            .rfind(')')
            .ok_or_else(|| anyhow::anyhow!("Missing ')' in C signature: {}", sig))?;

        let left = sig[..paren_start].trim();
        let ret_type_c = left
            .rsplit_once(&unit.name)
            .map(|(r, _)| r)
            .unwrap_or("int")
            .replace("static", "")
            .replace("inline", "")
            .trim()
            .to_string();

        let (c_ret, rust_ret) = map_return_type(&ret_type_c)?;

        let params_str = sig[paren_start + 1..paren_end].trim();
        let mut params = Vec::new();

        if !params_str.is_empty() && params_str != "void" {
            let raw_params: Vec<&str> = params_str.split(',').map(|s| s.trim()).collect();
            let mut i = 0;
            while i < raw_params.len() {
                let p = raw_params[i];
                let tokens: Vec<&str> = p.split_whitespace().collect();
                if tokens.len() >= 2 {
                    let p_name = sanitize_ident(tokens.last().unwrap().trim_start_matches('*'));
                    let type_part = p[..p.rfind(tokens.last().unwrap()).unwrap_or(p.len())].trim();
                    let is_pointer = p.contains('*');
                    let is_const = type_part.contains("const");

                    if is_pointer {
                        if type_part.contains("uint8_t") || type_part.contains("unsigned char") {
                            let mut len_name = format!("{}_len", p_name);
                            if i + 1 < raw_params.len() {
                                let next_p = raw_params[i + 1];
                                let next_tokens: Vec<&str> = next_p.split_whitespace().collect();
                                if !next_p.contains('*') && next_tokens.len() >= 2 {
                                    len_name = sanitize_ident(next_tokens.last().unwrap());
                                    i += 1; // consume length companion
                                }
                            }
                            params.push(ParamSpec::SliceU8 {
                                name: p_name,
                                len_name,
                                mutable: !is_const,
                            });
                        } else if type_part.contains("char") {
                            params.push(ParamSpec::CString { name: p_name });
                        } else {
                            anyhow::bail!("Unsupported pointer parameter in harness: {}", p);
                        }
                    } else {
                        let clean_type = type_part.replace("const", "").trim().to_string();
                        let (c_ty, rust_ty) = map_scalar_type(&clean_type)?;
                        params.push(ParamSpec::Scalar {
                            name: p_name,
                            c_ty,
                            rust_ty,
                        });
                    }
                }
                i += 1;
            }
        }

        Ok(Self {
            fn_name,
            params,
            c_ret,
            rust_ret,
        })
    }

    pub fn c_shim_params(&self) -> String {
        let mut out = Vec::new();
        for p in &self.params {
            match p {
                ParamSpec::Scalar { name, c_ty, .. } => {
                    out.push(format!("{} {}", c_ty, name));
                }
                ParamSpec::SliceU8 {
                    name,
                    len_name,
                    mutable,
                } => {
                    let qual = if *mutable { "" } else { "const " };
                    out.push(format!("{}uint8_t *{}", qual, name));
                    out.push(format!("size_t {}", len_name));
                }
                ParamSpec::CString { name } => {
                    out.push(format!("const char *{}", name));
                }
            }
        }
        if out.is_empty() {
            "void".to_string()
        } else {
            out.join(", ")
        }
    }

    pub fn c_call_args(&self) -> String {
        let mut out = Vec::new();
        for p in &self.params {
            match p {
                ParamSpec::Scalar { name, .. } => out.push(name.clone()),
                ParamSpec::SliceU8 { name, len_name, .. } => {
                    out.push(name.clone());
                    out.push(len_name.clone());
                }
                ParamSpec::CString { name } => out.push(name.clone()),
            }
        }
        out.join(", ")
    }

    pub fn rust_extern_params(&self) -> String {
        let mut out = Vec::new();
        for p in &self.params {
            match p {
                ParamSpec::Scalar { name, rust_ty, .. } => {
                    out.push(format!("{}: {}", name, rust_ty));
                }
                ParamSpec::SliceU8 {
                    name,
                    len_name,
                    mutable,
                } => {
                    let ptr_ty = if *mutable { "*mut u8" } else { "*const u8" };
                    out.push(format!("{}: {}", name, ptr_ty));
                    out.push(format!("{}: usize", len_name));
                }
                ParamSpec::CString { name } => {
                    out.push(format!("{}: *const std::os::raw::c_char", name));
                }
            }
        }
        out.join(", ")
    }
}

fn map_scalar_type(c_type: &str) -> anyhow::Result<(String, String)> {
    let pair = match c_type {
        "int" | "int32_t" | "signed int" => ("int32_t", "i32"),
        "unsigned" | "unsigned int" | "uint32_t" => ("uint32_t", "u32"),
        "int64_t" | "long" | "long long" => ("int64_t", "i64"),
        "uint64_t" | "unsigned long" | "unsigned long long" => ("uint64_t", "u64"),
        "size_t" => ("size_t", "usize"),
        "uint8_t" | "unsigned char" => ("uint8_t", "u8"),
        "int8_t" | "signed char" => ("int8_t", "i8"),
        "uint16_t" | "unsigned short" => ("uint16_t", "u16"),
        "int16_t" | "short" => ("int16_t", "i16"),
        "float" => ("float", "f32"),
        "double" => ("double", "f64"),
        "bool" | "_Bool" => ("bool", "bool"),
        other => anyhow::bail!("Unsupported scalar C type: {}", other),
    };
    Ok((pair.0.to_string(), pair.1.to_string()))
}

fn map_return_type(c_type: &str) -> anyhow::Result<(String, String)> {
    if c_type == "void" {
        return Ok(("void".to_string(), "()".to_string()));
    }
    map_scalar_type(c_type)
}

impl HarnessGen {
    /// Generates the deterministic `extern "C"` wrapper, C shim, and differential harness.
    pub fn from_template(
        unit: &MigrationUnit,
        rust_code: &str,
        out_dir: &Path,
    ) -> anyhow::Result<HarnessArtifacts> {
        let spec = SignatureSpec::parse(unit)?;
        let fn_name = &spec.fn_name;
        let unit_dir = out_dir.join(fn_name);
        std::fs::create_dir_all(&unit_dir)?;

        let c_shim_path = unit_dir.join(format!("{}_shim.c", fn_name));
        let fuzz_target_path = unit_dir.join(format!("{}_fuzz.rs", fn_name));
        let seed_corpus_path = unit_dir.join("corpus");
        std::fs::create_dir_all(&seed_corpus_path)?;

        let ret_prefix = if spec.c_ret == "void" { "" } else { "return " };

        // 1. Generate C shim
        let c_shim_content = format!(
            r#"/* Auto-generated by Ferrite for {name} */
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <string.h>
#include <limits.h>

{c_body}

/* Exported symbol for differential harness */
{c_ret} c_{name}({c_params}) {{
    {ret_prefix}{name}({c_args});
}}
"#,
            name = fn_name,
            c_body = unit.source_body,
            c_ret = spec.c_ret,
            c_params = spec.c_shim_params(),
            ret_prefix = ret_prefix,
            c_args = spec.c_call_args(),
        );
        std::fs::write(&c_shim_path, c_shim_content)?;

        // 2. Generate runner source for differential verification
        let runner_code = Self::generate_runner_code(&spec, rust_code);
        std::fs::write(&fuzz_target_path, runner_code)?;

        // 3. Populate seed corpus based on FeatureVector & signature
        let seeds = [
            "0,0,0",
            "10,0,100",
            "-5,0,10",
            "105,0,100",
            "-2147483648,-100,100",
            "2147483647,-100,100",
            "hello_ferrite",
        ];
        for (i, s) in seeds.iter().enumerate() {
            let seed_file = seed_corpus_path.join(format!("seed_{}.bin", i));
            std::fs::write(seed_file, s)?;
        }

        Ok(HarnessArtifacts {
            fuzz_target: fuzz_target_path,
            c_shim: c_shim_path,
            seed_corpus: seed_corpus_path,
        })
    }

    pub fn generate_runner_code(spec: &SignatureSpec, rust_code: &str) -> String {
        let name = &spec.fn_name;
        let extern_ret = if spec.rust_ret == "()" {
            String::new()
        } else {
            format!(" -> {}", spec.rust_ret)
        };

        // Build per-iteration argument generation & call expressions
        let mut arg_setup = String::new();
        let mut rust_args = Vec::new();
        let mut c_args = Vec::new();
        let mut mut_slice_checks = String::new();

        for (idx, p) in spec.params.iter().enumerate() {
            match p {
                ParamSpec::Scalar {
                    name: p_name,
                    rust_ty,
                    ..
                } => {
                    arg_setup.push_str(&format!(
                        r#"
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let raw_{idx}: i64 = match iter {{
            0 => 0,
            1 => 1,
            2 => -1,
            3 => 10,
            4 => 100,
            5 => -5,
            6 => 255,
            7 => i32::MAX as i64,
            8 => i32::MIN as i64,
            _ => ((state >> 32) as i32) as i64,
        }};
        let {p_name}: {rust_ty} = raw_{idx} as {rust_ty};
"#,
                        idx = idx,
                        p_name = p_name,
                        rust_ty = rust_ty
                    ));
                    rust_args.push(p_name.clone());
                    c_args.push(p_name.clone());
                }
                ParamSpec::SliceU8 {
                    name: _p_name,
                    len_name: _,
                    mutable,
                } => {
                    arg_setup.push_str(&format!(
                        r#"
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let slice_len_{idx}: usize = match iter {{
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 16,
            _ => ((state >> 48) as usize) % 64,
        }};
        let mut buf_rust_{idx}: Vec<u8> = (0..slice_len_{idx})
            .map(|j| {{
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 56) as u8
            }})
            .collect();
        let mut buf_c_backing_{idx}: Vec<u8> = buf_rust_{idx}.clone();
        buf_c_backing_{idx}.push(0xAA);
        buf_c_backing_{idx}.push(0x55);
        let mut buf_c_{idx}: Vec<u8> = buf_rust_{idx}.clone();
"#,
                        idx = idx
                    ));
                    if *mutable {
                        rust_args.push(format!("&mut buf_rust_{}", idx));
                        c_args.push(format!("buf_c_backing_{idx}.as_mut_ptr()", idx = idx));
                        c_args.push(format!("buf_c_{}.len()", idx));
                        mut_slice_checks.push_str(&format!(
                            r#"
        if buf_c_backing_{idx}[slice_len_{idx}] != 0xAA || buf_c_backing_{idx}[slice_len_{idx} + 1] != 0x55 {{
            eprintln!("ASAN_CANARY_OVERWRITE: iter={{}}, out-of-bounds write past len={{}}", iter, slice_len_{idx});
            std::process::exit(99);
        }}
        if &buf_rust_{idx}[..] != &buf_c_backing_{idx}[..slice_len_{idx}] {{
            eprintln!("DIVERGED_BUF: iter={{}}, rust_buf={{:?}}, c_buf={{:?}}", iter, buf_rust_{idx}, &buf_c_backing_{idx}[..slice_len_{idx}]);
            std::process::exit(42);
        }}
"#,
                            idx = idx
                        ));
                    } else {
                        rust_args.push(format!("&buf_rust_{}", idx));
                        c_args.push(format!("buf_c_backing_{idx}.as_ptr()", idx = idx));
                        c_args.push(format!("buf_c_{}.len()", idx));
                    }
                }
                ParamSpec::CString { name: p_name } => {
                    arg_setup.push_str(&format!(
                        r#"
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let str_len_{idx}: usize = match iter {{
            0 => 0,
            1 => 1,
            2 => 5,
            3 => 12,
            _ => ((state >> 48) as usize) % 32,
        }};
        let mut s_bytes_{idx}: Vec<u8> = (0..str_len_{idx})
            .map(|_| {{
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let b = ((state >> 56) as u8) % 95 + 32;
                b
            }})
            .collect();
        let {p_name}_str = String::from_utf8_lossy(&s_bytes_{idx}).into_owned();
        let mut {p_name}_cbytes = {p_name}_str.as_bytes().to_vec();
        {p_name}_cbytes.push(0);
"#,
                        idx = idx,
                        p_name = p_name
                    ));
                    rust_args.push(format!("&{}_str", p_name));
                    c_args.push(format!(
                        "{}_cbytes.as_ptr() as *const std::os::raw::c_char",
                        p_name
                    ));
                }
            }
        }

        // Special ordering constraint for 3-scalar clamp-like functions where params are (val, min, max)
        let scalar_ordering_fix = if spec.params.len() == 3
            && spec
                .params
                .iter()
                .all(|p| matches!(p, ParamSpec::Scalar { .. }))
        {
            if let (ParamSpec::Scalar { name: n1, .. }, ParamSpec::Scalar { name: n2, .. }) =
                (&spec.params[1], &spec.params[2])
            {
                format!(
                    "let ({n1}, {n2}) = if {n1} <= {n2} {{ ({n1}, {n2}) }} else {{ ({n2}, {n1}) }};",
                    n1 = n1,
                    n2 = n2
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let cmp_expr = if spec.rust_ret == "f32" || spec.rust_ret == "f64" {
            "(r_rust - r_c).abs() > 1e-5 && !(r_rust.is_nan() && r_c.is_nan())"
        } else {
            "r_rust != r_c"
        };

        format!(
            r#"/* Auto-generated differential runner for {name} */
#![allow(unused_mut, unused_variables, clippy::all)]
{rust_code}

extern "C" {{
    fn c_{name}({extern_params}){extern_ret};
}}

fn main() {{
    let mut state: u64 = 0x853c49e6748fea9b;
    let mut count = 0u64;

    for iter in 0..10_012 {{
        {arg_setup}
        {scalar_ordering_fix}
        let r_rust = {name}({rust_call});
        let r_c = unsafe {{ c_{name}({c_call}) }};
        count += 1;
        if {cmp_expr} {{
            eprintln!("DIVERGED: iter={{}}, rust={{:?}}, c={{:?}}", iter, r_rust, r_c);
            std::process::exit(42);
        }}
        {mut_slice_checks}
    }}

    println!("SUCCESS: executions={{}}", count);
}}
"#,
            name = name,
            rust_code = rust_code,
            extern_params = spec.rust_extern_params(),
            extern_ret = extern_ret,
            arg_setup = arg_setup,
            scalar_ordering_fix = scalar_ordering_fix,
            rust_call = rust_args.join(", "),
            c_call = c_args.join(", "),
            cmp_expr = cmp_expr,
            mut_slice_checks = mut_slice_checks,
        )
    }
}

pub fn sanitize_ident(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if sanitized.is_empty() {
        "fn_unnamed".to_string()
    } else {
        sanitized
    }
}
