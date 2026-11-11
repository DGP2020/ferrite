# ADR-0009: Project Manifest Schema (`ferrite.toml`)

**Date**: 2026-10-22
**Status**: accepted
**Deciders**: Ferrite Architecture Team

## Context

Real-world C libraries and legacy applications rely on diverse build systems, including CMake, GNU Autotools, hand-written Makefiles, Meson, Ninja, and custom shell scripts. Build rules often incorporate complex compiler flags, non-standard header include directories, architectural macros, and target dependencies.

Attempting to heuristically reverse-engineer arbitrary build systems to discover include paths, preprocessor flags, and compilation flags is inherently brittle and prone to silent failures. Furthermore, users need a way to skip functions with unsupported constructs and define manual clustering overrides for functions sharing internal static state.

## Decision

Standardize target project configuration around an explicit, human-readable, and version-controlled configuration manifest file: `ferrite.toml`.

The manifest specifies:
- `version = 1`: Schema versioning for backward compatibility.
- `[library]`: Name, optional `compile_commands.json` path, include search directories (`include_dirs`), and source file globs (`source_files`).
- `build_cmd` and `test_cmd`: Native project commands invoked during Stage 6 (L4 Hybrid Integration).
- `defines`: Preprocessor macro flags passed to Clang during AST extraction and harness compilation.
- `skip`: Array of function names explicitly excluded from translation.
- `[clusters]`: Groupings of functions that must be analyzed, translated, and verified together as a single composite `MigrationUnit`.

## Alternatives Considered

### Alternative 1: Automatic Build System Inference (Heuristic Make/CMake Parsing)
- **Pros**: Zero initial setup for standard, well-structured projects.
- **Cons**: Fragile failure modes on custom build scripts, cross-compilation configurations, or non-standard directory layouts.
- **Why not**: Generates subtle compilation errors due to missing preprocessor flags or header search paths that are difficult for users to diagnose.

### Alternative 2: Command-Line Flags Only (`--include`, `--define`, `--build-cmd`)
- **Pros**: Avoids adding a configuration file to the target repository.
- **Cons**: Results in cumbersome, error-prone CLI invocations with dozens of flags; difficult to share or reproduce across team members and CI pipelines.
- **Why not**: Poor developer ergonomics and non-reproducible CI execution.

## Consequences

### Positive
- Clear, deterministic, and self-documenting project configuration tracked in version control.
- Explicit support for function clusters, preprocessor definitions, and skip lists.
- Decouples Ferrite from specific C build engines (Make, CMake, Meson) by invoking clean user-specified build and test commands.

### Negative
- Requires repository authors or migration engineers to author a minimal `ferrite.toml` file before running Ferrite.
- Configuration can become out-of-date if source files are moved without updating `ferrite.toml`.

### Risks
- Users may forget required preprocessor defines; mitigated by providing `ferrite init` scaffolding and surfacing Clang diagnostic warnings when unresolved macros are encountered.
