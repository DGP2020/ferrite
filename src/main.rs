use clap::{Parser, Subcommand};
use std::path::PathBuf;

use ferrite::bench::{BenchManifest, BenchRunner};
use ferrite::dashboard::Dashboard;
use ferrite::models::{FunctionReport, LibraryConfig, LibraryMeta, UnitStatus};
use ferrite::policy::{CascadePolicy, StaticPolicy};
use ferrite::provider::provider_from_env;
use ferrite::translate::Translator;
use ferrite::verify::Verifier;

#[derive(Parser, Debug)]
#[command(name = "ferrite", about = "Verified C-to-Rust migration agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to a directory or file of C source files to migrate.
    #[arg(long, default_value = "fixtures/clamp")]
    source_dir: PathBuf,

    /// Optional path to ferrite.toml manifest.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Where to write generated harnesses, corpus, and reports.
    #[arg(long, default_value = "ferrite-out")]
    work_dir: PathBuf,

    /// Which model policy to use.
    #[arg(long, value_enum, default_value = "static")]
    policy: PolicyChoice,

    /// Which translation provider to use (`replay`, `mock`, or `openai`).
    #[arg(long, value_enum, default_value = "replay")]
    provider: ProviderChoice,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run Ferrite-Bench Lite manifest and emit bench-results.json + leaderboard table.
    Bench {
        /// Path to the benchmark manifest TOML.
        #[arg(long, default_value = "bench/manifest.toml")]
        manifest: PathBuf,

        /// Where to write bench-results.json and generated harnesses.
        #[arg(long, default_value = "ferrite-out")]
        work_dir: PathBuf,

        /// Which translation provider to use (`replay`, `mock`, or `openai`).
        #[arg(long, value_enum, default_value = "replay")]
        provider: ProviderChoice,
    },
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum PolicyChoice {
    Static,
    Cascade,
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum ProviderChoice {
    Replay,
    Mock,
    Openai,
}

impl ProviderChoice {
    fn as_str(&self) -> &'static str {
        match self {
            ProviderChoice::Replay => "replay",
            ProviderChoice::Mock => "mock",
            ProviderChoice::Openai => "openai",
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Commands::Bench {
        manifest,
        work_dir,
        provider,
    }) = cli.command
    {
        std::fs::create_dir_all(&work_dir)?;
        let provider_impl = provider_from_env(Some(provider.as_str()))?;
        let bench_manifest = BenchManifest::load(&manifest)?;
        let runner = BenchRunner::new(provider_impl, &work_dir);
        let summary = runner.run_manifest(&bench_manifest, std::path::Path::new("."))?;
        println!("{}", summary.format_leaderboard());
        return Ok(());
    }

    std::fs::create_dir_all(&cli.work_dir)?;

    // 1. Load library configuration (ADR 0009)
    let config = load_or_default_config(&cli)?;

    eprintln!(
        "\x1b[1;34m[Ferrite]\x1b[0m Starting migration run for library '{}' (provider: {})...",
        config.library.name,
        cli.provider.as_str()
    );

    // 2. Analyze C source files (ADR 0002)
    let analyzer = ferrite::analyze::Analyzer;
    let units = analyzer.analyze_dir(&cli.source_dir)?;
    if units.is_empty() {
        eprintln!(
            "\x1b[1;33m[Warning]\x1b[0m No C functions found in {:?}",
            cli.source_dir
        );
        return Ok(());
    }
    eprintln!(
        "\x1b[1;32m[Analyze]\x1b[0m Extracted {} migration unit(s).",
        units.len()
    );

    // 3. Plan bottom-up execution order
    let planned_units = ferrite::plan::Planner::order(units);

    // 4. Set up Policy, Provider, Translator, and Verifier
    let policy: Box<dyn ferrite::policy::ModelPolicy> = match cli.policy {
        PolicyChoice::Static => Box::new(StaticPolicy::default()),
        PolicyChoice::Cascade => Box::new(CascadePolicy::default()),
    };
    let provider = provider_from_env(Some(cli.provider.as_str()))?;
    let translator = Translator::new(policy, provider);
    let verifier = Verifier::new(&cli.work_dir);

    // 5. Execute translation and verification pipeline per MigrationUnit
    let mut reports = Vec::new();

    for mut unit in planned_units {
        eprintln!(
            "\x1b[1;36m[Translate]\x1b[0m Generating Rust for '{}'...",
            unit.name
        );
        unit.status = UnitStatus::Translating;

        let attempt = translator.translate_one(&unit, &[])?;
        if !attempt.compile_result.ok {
            eprintln!(
                "\x1b[1;31m[Compile Failed]\x1b[0m Rust compilation failed for '{}'",
                unit.name
            );
            unit.status = UnitStatus::CompileFailed;
            reports.push(FunctionReport {
                unit,
                verdict: attempt.verdict.unwrap_or(ferrite::models::Verdict {
                    kind: ferrite::models::VerdictKind::CompileFailed,
                    assurance_level: None,
                    executions: 0,
                    coverage_c_percent: 0.0,
                    coverage_rust_percent: 0.0,
                    coverage_plateaued: false,
                    miri_ok: None,
                    counterexample: None,
                    asan_report: None,
                    notes: attempt.compile_result.errors.join("\n"),
                }),
                attempt_count: 1,
                total_tokens: attempt.tokens,
                total_cost_usd: attempt.cost_usd.unwrap_or(0.0),
                total_latency_ms: attempt.latency_ms.unwrap_or(0),
                unsafe_line_count: 0,
            });
            continue;
        }

        eprintln!(
            "\x1b[1;35m[Verify]\x1b[0m Running differential fuzz campaign for '{}'...",
            unit.name
        );
        unit.status = UnitStatus::Verifying;

        let verdict = verifier.verify(&unit, &attempt.rust_code)?;
        unit.assurance_level = verdict.assurance_level;
        unit.status = match verdict.kind {
            ferrite::models::VerdictKind::Equivalent => UnitStatus::Verified,
            ferrite::models::VerdictKind::Diverged => UnitStatus::Diverged,
            ferrite::models::VerdictKind::CLatentBug => UnitStatus::CLatentBug,
            _ => UnitStatus::TranslatingFailed,
        };

        // Audit unsafe lines via syn
        let (_, unsafe_count) =
            ferrite::translate::validate_rust_ast(&attempt.rust_code, &unit.name)?;

        reports.push(FunctionReport {
            unit,
            verdict,
            attempt_count: 1,
            total_tokens: attempt.tokens,
            total_cost_usd: attempt.cost_usd.unwrap_or(0.0),
            total_latency_ms: attempt.latency_ms.unwrap_or(0),
            unsafe_line_count: unsafe_count,
        });
    }

    // 6. Render Terminal Dashboard
    Dashboard::render_terminal(&config, &reports);

    // 7. Write Standalone HTML Dashboard
    let html_path = cli.work_dir.join("dashboard.html");
    Dashboard::write_html(&config, &reports, &html_path)?;
    println!(
        "* Interactive HTML dashboard saved to: {}",
        html_path.display()
    );
    println!();

    Ok(())
}

fn load_or_default_config(cli: &Cli) -> anyhow::Result<LibraryConfig> {
    let manifest_path = if let Some(ref path) = cli.config {
        path.clone()
    } else {
        cli.source_dir.join("ferrite.toml")
    };

    if manifest_path.exists() {
        let content = std::fs::read_to_string(&manifest_path)?;
        let config: LibraryConfig = toml::from_str(&content)?;
        Ok(config)
    } else {
        let dir_name = cli
            .source_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("project")
            .to_string();

        Ok(LibraryConfig {
            version: 1,
            library: LibraryMeta {
                name: dir_name,
                compile_commands: None,
                include_dirs: vec![],
                source_files: vec![],
                build_cmd: None,
                test_cmd: None,
            },
            clusters: std::collections::HashMap::new(),
            defines: vec![],
            skip: vec![],
        })
    }
}
