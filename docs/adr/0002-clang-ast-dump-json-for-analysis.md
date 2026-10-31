# Clang JSON AST Dump for C Semantic Analysis

Tree-sitter cannot expand macros or resolve types across headers, while direct libclang FFI bindings require local LLVM development headers and fragile platform-specific linking. We invoke `clang -Xclang -ast-dump=json` filtered to the target file's AST nodes and parse the schema with `serde`, retrieving macro definitions separately via `clang -E -dM`.
