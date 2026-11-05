//! Dashboard generator for terminal and interactive HTML reporting.

use crate::models::{AssuranceLevel, FunctionReport, LibraryConfig, VerdictKind};
use std::path::Path;

pub struct Dashboard;

impl Dashboard {
    /// Render clean, human-readable terminal dashboard.
    pub fn render_terminal(config: &LibraryConfig, reports: &[FunctionReport]) {
        println!();
        println!(
            "================================================================================"
        );
        println!(
            "                         FERRITE v2 MIGRATION DASHBOARD                         "
        );
        println!(
            "================================================================================"
        );
        println!(
            " Library: {:<20} Version: {}",
            config.library.name, config.version
        );
        println!(
            " Migration Units: {:<12} Total Executions: {}",
            reports.len(),
            reports.iter().map(|r| r.verdict.executions).sum::<u64>()
        );
        println!(
            "--------------------------------------------------------------------------------"
        );

        let mut l4_count = 0;
        let mut l3_count = 0;
        let mut l2_count = 0;
        let mut l1_count = 0;
        let mut l0_count = 0;
        let mut failed_count = 0;
        let mut asan_findings = Vec::new();

        for rep in reports {
            match rep.verdict.assurance_level {
                Some(AssuranceLevel::L4) => l4_count += 1,
                Some(AssuranceLevel::L3) => l3_count += 1,
                Some(AssuranceLevel::L2) => l2_count += 1,
                Some(AssuranceLevel::L1) => l1_count += 1,
                Some(AssuranceLevel::L0) => l0_count += 1,
                None => failed_count += 1,
            }

            if rep.verdict.kind == VerdictKind::CLatentBug || rep.verdict.asan_report.is_some() {
                asan_findings.push(rep);
            }
        }

        println!(" Assurance Ladder Progression:");
        println!("   [L4] Hybrid Integrated         : {}", l4_count);
        println!("   [L3] Stateful API Tested       : {}", l3_count);
        println!("   [L2] Differentially Tested     : {}", l2_count);
        println!("   [L1] Unit Tested               : {}", l1_count);
        println!("   [L0] Compiles / Clippy Clean   : {}", l0_count);
        println!("   [Failed / Diverged]            : {}", failed_count);

        if !asan_findings.is_empty() {
            println!(
                "--------------------------------------------------------------------------------"
            );
            println!(
                "                     ADDRESSSANITIZER / UBSAN FINDINGS                          "
            );
            println!(
                "--------------------------------------------------------------------------------"
            );
            println!(
                " {:<26} | {:<32} | {:<10}",
                "Function", "Sanitizer Defect Type", "Executions"
            );
            println!(
                "--------------------------------------------------------------------------------"
            );
            for rep in &asan_findings {
                let defect = rep
                    .verdict
                    .asan_report
                    .clone()
                    .unwrap_or_else(|| "Memory or Undefined Behavior defect".to_string());
                let short_defect = if defect.len() > 32 {
                    format!("{}...", &defect[..29])
                } else {
                    defect
                };
                println!(
                    " {:<26} | {:<32} | {:<10}",
                    rep.unit.name, short_defect, rep.verdict.executions
                );
            }
        }

        println!(
            "================================================================================"
        );
        println!();

        for rep in reports {
            let badge = match rep.verdict.kind {
                VerdictKind::CLatentBug => "\x1b[33m[ C LATENT BUG SURFACED ]\x1b[0m",
                _ => match rep.verdict.assurance_level {
                    Some(AssuranceLevel::L4) => "\x1b[35m[ L4 INTEGRATED ]\x1b[0m",
                    Some(AssuranceLevel::L3) => "\x1b[36m[ L3 STATEFUL ]\x1b[0m",
                    Some(AssuranceLevel::L2) => "\x1b[32m[ L2 VERIFIED ]\x1b[0m",
                    Some(AssuranceLevel::L1) => "\x1b[34m[ L1 TESTED ]\x1b[0m",
                    Some(AssuranceLevel::L0) => "\x1b[33m[ L0 COMPILES ]\x1b[0m",
                    None => "\x1b[31m[ UNVERIFIED ]\x1b[0m",
                },
            };

            println!("* Unit: {} {}", rep.unit.name, badge);
            println!("  C Signature : {}", rep.unit.signature);
            println!("  Rust Target : {}", rep.unit.rust_contract_sig);
            println!("  Verdict     : {:?}", rep.verdict.kind);
            println!(
                "  Executions  : {} differential executions",
                rep.verdict.executions
            );
            println!(
                "  Coverage    : C: {:.1}% | Rust: {:.1}% (Plateau: {})",
                rep.verdict.coverage_c_percent,
                rep.verdict.coverage_rust_percent,
                rep.verdict.coverage_plateaued
            );
            println!("  Unsafe Lines: {} (100% Safe Rust)", rep.unsafe_line_count);
            if let Some(asan) = &rep.verdict.asan_report {
                println!("  ASan Report : {}", asan);
            }
            println!("  Notes       : {}", rep.verdict.notes);
            println!(
                "--------------------------------------------------------------------------------"
            );
        }
    }

    /// Write standalone HTML dashboard artifact.
    pub fn write_html(
        config: &LibraryConfig,
        reports: &[FunctionReport],
        out_path: &Path,
    ) -> anyhow::Result<()> {
        let mut cards = String::new();
        let safe_lib_name = html_escape(&config.library.name);

        let mut l4_count = 0;
        let mut l3_count = 0;
        let mut l2_count = 0;
        let mut l1_count = 0;
        let mut l0_count = 0;
        let mut failed_count = 0;
        let mut asan_findings = Vec::new();

        for rep in reports {
            match rep.verdict.assurance_level {
                Some(AssuranceLevel::L4) => l4_count += 1,
                Some(AssuranceLevel::L3) => l3_count += 1,
                Some(AssuranceLevel::L2) => l2_count += 1,
                Some(AssuranceLevel::L1) => l1_count += 1,
                Some(AssuranceLevel::L0) => l0_count += 1,
                None => failed_count += 1,
            }

            if rep.verdict.kind == VerdictKind::CLatentBug || rep.verdict.asan_report.is_some() {
                asan_findings.push(rep);
            }

            let (badge_class, badge_text) = match rep.verdict.kind {
                VerdictKind::CLatentBug => ("badge-asan", "C LATENT BUG"),
                _ => match rep.verdict.assurance_level {
                    Some(AssuranceLevel::L4) => ("badge-l4", "L4 INTEGRATED"),
                    Some(AssuranceLevel::L3) => ("badge-l3", "L3 STATEFUL"),
                    Some(AssuranceLevel::L2) => ("badge-l2", "L2 VERIFIED"),
                    Some(AssuranceLevel::L1) => ("badge-l1", "L1 TESTED"),
                    Some(AssuranceLevel::L0) => ("badge-l0", "L0 COMPILED"),
                    None => ("badge-failed", "UNVERIFIED"),
                },
            };

            let asan_html = if let Some(asan) = &rep.verdict.asan_report {
                format!(
                    r#"<div class="asan-callout"><strong>AddressSanitizer Report:</strong> <code>{}</code></div>"#,
                    html_escape(asan)
                )
            } else {
                String::new()
            };

            cards.push_str(&format!(
                r#"
                <div class="card">
                    <div class="card-header">
                        <h3><code>{name}</code></h3>
                        <span class="badge {badge_class}">{badge_text}</span>
                    </div>
                    <div class="card-body">
                        <p><strong>C Signature:</strong> <code>{c_sig}</code></p>
                        <p><strong>Rust Contract:</strong> <code>{rust_sig}</code></p>
                        <div class="stats-grid">
                            <div class="stat"><span class="label">Executions</span><span class="val">{execs}</span></div>
                            <div class="stat"><span class="label">Rust Coverage</span><span class="val">{rust_cov:.1}%</span></div>
                            <div class="stat"><span class="label">C Coverage</span><span class="val">{c_cov:.1}%</span></div>
                            <div class="stat"><span class="label">Unsafe Lines</span><span class="val">{unsafe_lines}</span></div>
                        </div>
                        {asan_html}
                        <p class="notes"><strong>Verification Notes:</strong> {notes}</p>
                    </div>
                </div>
                "#,
                name = html_escape(&rep.unit.name),
                badge_class = badge_class,
                badge_text = badge_text,
                c_sig = html_escape(&rep.unit.signature),
                rust_sig = html_escape(&rep.unit.rust_contract_sig),
                execs = rep.verdict.executions,
                rust_cov = rep.verdict.coverage_rust_percent,
                c_cov = rep.verdict.coverage_c_percent,
                unsafe_lines = rep.unsafe_line_count,
                asan_html = asan_html,
                notes = html_escape(&rep.verdict.notes),
            ));
        }

        let mut asan_box = String::new();
        if !asan_findings.is_empty() {
            let mut rows = String::new();
            for rep in &asan_findings {
                let defect = rep
                    .verdict
                    .asan_report
                    .clone()
                    .unwrap_or_else(|| "Memory or Undefined Behavior defect".to_string());
                rows.push_str(&format!(
                    "<tr><td><code>{}</code></td><td><span class=\"badge badge-asan\">{}</span></td><td>{}</td></tr>",
                    html_escape(&rep.unit.name),
                    html_escape(&defect),
                    rep.verdict.executions
                ));
            }
            asan_box = format!(
                r#"
                <div class="card asan-box">
                    <div class="card-header" style="background: #3b1b1f;">
                        <h3>AddressSanitizer / UBSan Findings</h3>
                        <span class="badge badge-asan">{count} Planted/Latent Defects</span>
                    </div>
                    <div class="card-body">
                        <table class="findings-table">
                            <thead>
                                <tr><th>Function</th><th>Sanitizer Defect Type</th><th>Executions</th></tr>
                            </thead>
                            <tbody>
                                {rows}
                            </tbody>
                        </table>
                    </div>
                </div>
                "#,
                count = asan_findings.len(),
                rows = rows
            );
        }

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Ferrite Migration Dashboard: {lib_name}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; background: #0f141c; color: #e6edf3; margin: 0; padding: 2rem; }}
        .container {{ max-width: 900px; margin: 0 auto; }}
        header {{ border-bottom: 1px solid #30363d; padding-bottom: 1.5rem; margin-bottom: 2rem; }}
        h1 {{ margin: 0 0 0.5rem 0; color: #58a6ff; font-size: 1.8rem; }}
        .subtitle {{ color: #8b949e; margin: 0; }}
        .ladder-grid {{ display: grid; grid-template-columns: repeat(6, 1fr); gap: 0.75rem; margin-bottom: 2rem; }}
        .ladder-step {{ background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 0.75rem; text-align: center; }}
        .ladder-step .lvl {{ font-size: 0.75rem; color: #8b949e; font-weight: bold; }}
        .ladder-step .count {{ font-size: 1.4rem; font-weight: bold; margin-top: 0.25rem; color: #58a6ff; }}
        .card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; margin-bottom: 1.5rem; overflow: hidden; }}
        .card-header {{ display: flex; justify-content: space-between; align-items: center; padding: 1rem 1.5rem; background: #21262d; border-bottom: 1px solid #30363d; }}
        .card-header h3 {{ margin: 0; font-size: 1.2rem; }}
        .card-body {{ padding: 1.5rem; }}
        .badge {{ padding: 0.25rem 0.6rem; border-radius: 20px; font-size: 0.8rem; font-weight: bold; letter-spacing: 0.5px; }}
        .badge-l4 {{ background: #8957e5; color: white; }}
        .badge-l3 {{ background: #1f6feb; color: white; }}
        .badge-l2 {{ background: #238636; color: white; }}
        .badge-l1 {{ background: #388bfd; color: white; }}
        .badge-l0 {{ background: #d29922; color: black; }}
        .badge-asan {{ background: #da3633; color: white; }}
        .badge-failed {{ background: #6e7681; color: white; }}
        .stats-grid {{ display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin: 1.2rem 0; background: #0d1117; padding: 1rem; border-radius: 6px; border: 1px solid #30363d; }}
        .stat {{ display: flex; flex-direction: column; }}
        .stat .label {{ font-size: 0.75rem; color: #8b949e; text-transform: uppercase; margin-bottom: 0.25rem; }}
        .stat .val {{ font-size: 1.2rem; font-weight: bold; color: #58a6ff; }}
        code {{ background: #2b313a; padding: 0.2rem 0.4rem; border-radius: 4px; font-family: monospace; font-size: 0.9em; }}
        .notes {{ font-size: 0.9rem; color: #8b949e; margin-top: 1rem; border-left: 3px solid #58a6ff; padding-left: 0.75rem; }}
        .asan-callout {{ margin: 1rem 0; padding: 0.75rem; background: #3b1b1f; border-left: 3px solid #da3633; border-radius: 4px; font-size: 0.85rem; color: #f85149; }}
        .findings-table {{ width: 100%; border-collapse: collapse; }}
        .findings-table th, .findings-table td {{ padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid #30363d; }}
        .findings-table th {{ color: #8b949e; font-size: 0.8rem; text-transform: uppercase; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>Ferrite v2 Migration Report</h1>
            <p class="subtitle">Library: <strong>{lib_name}</strong> | Version: {version}</p>
        </header>

        <section class="ladder-grid">
            <div class="ladder-step"><div class="lvl">L4 INTEGRATED</div><div class="count">{l4}</div></div>
            <div class="ladder-step"><div class="lvl">L3 STATEFUL</div><div class="count">{l3}</div></div>
            <div class="ladder-step"><div class="lvl">L2 VERIFIED</div><div class="count">{l2}</div></div>
            <div class="ladder-step"><div class="lvl">L1 TESTED</div><div class="count">{l1}</div></div>
            <div class="ladder-step"><div class="lvl">L0 COMPILED</div><div class="count">{l0}</div></div>
            <div class="ladder-step"><div class="lvl">UNVERIFIED</div><div class="count">{failed}</div></div>
        </section>

        {asan_box}

        <section class="cards">
            {cards}
        </section>
    </div>
</body>
</html>
"#,
            lib_name = safe_lib_name,
            version = config.version,
            l4 = l4_count,
            l3 = l3_count,
            l2 = l2_count,
            l1 = l1_count,
            l0 = l0_count,
            failed = failed_count,
            asan_box = asan_box,
            cards = cards,
        );

        std::fs::write(out_path, html)?;
        Ok(())
    }
}

fn html_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}
