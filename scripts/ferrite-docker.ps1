<#
.SYNOPSIS
    Builds and runs Ferrite tests and Ferrite-Bench Lite inside an isolated Linux Docker container (no network).
#>
param(
    [ValidateSet("test", "bench", "all")]
    [string]$Mode = "all"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$ImageTag = "ferrite-verify:latest"

Write-Host "[Ferrite Docker] Building verification image '$ImageTag'..." -ForegroundColor Cyan
docker build -t $ImageTag -f "$RepoRoot/docker/Dockerfile" "$RepoRoot"

if ($Mode -eq "test" -or $Mode -eq "all") {
    Write-Host "[Ferrite Docker] Running cargo fmt, clippy, and cargo test (offline, --network none)..." -ForegroundColor Green
    docker run --rm --network none `
        -v "${RepoRoot}:/workspace" `
        -w /workspace `
        $ImageTag `
        sh -c "cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
}

if ($Mode -eq "bench" -or $Mode -eq "all") {
    Write-Host "[Ferrite Docker] Running ferrite bench in replay mode (--network none)..." -ForegroundColor Magenta
    docker run --rm --network none `
        -v "${RepoRoot}:/workspace" `
        -w /workspace `
        $ImageTag `
        cargo run -- bench --manifest bench/manifest.toml --provider replay
}
