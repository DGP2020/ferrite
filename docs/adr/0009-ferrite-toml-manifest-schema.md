# Project Manifest Schema (`ferrite.toml`)

To specify how target libraries are built, tested, and clustered without brittle build-system reverse engineering, Ferrite consumes a per-library `ferrite.toml` manifest containing `version = 1`, optional `compile_commands` path, source and include directories, build and test commands, preprocessor `defines`, a `skip` list, and manual cluster overrides.
