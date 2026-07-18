# Task 1 Review Package — Theme Library and Legacy Migration

## Repository condition

This workspace has no `.git` directory, so no `git diff` baseline is available. Review the current contents of the listed files as the implementation under review.

## Task contract

Changed files must be limited to:

- `src-tauri/src/models.rs`
- `src-tauri/src/storage/themes.rs`
- `src-tauri/src/storage/mod.rs`

Required behavior:

1. `ThemeSource::{Builtin, Wallpaper}`, `ThemeLayers`, `ThemeLibrary`, and `THEME_LIBRARY_VERSION` are serde-compatible and camel-case serialized.
2. `Theme` retains legacy visible fields and adds `source` and `layers`.
3. Built-ins are `Builtin`, have no wallpaper, and all layers are in `0.0..=1.0`.
4. `themes.json` is the current store. Loading when absent migrates legacy `settings.json` without altering it.
5. A legacy selection of an inbuilt theme with `backgroundImage` produces a newly selected `Wallpaper` theme which retains background path, selected base colors, and layer settings.
6. Future library versions return `CommandError` code `theme_library_version_unsupported`.
7. Existing callers must still compile until Task 4 replaces old settings commands.
8. No additional dependency or non-loopback network behavior.

## Required validation

```powershell
cargo test --manifest-path src-tauri\Cargo.toml storage::themes models::tests
cargo check --manifest-path src-tauri\Cargo.toml
```

## Reviewer verdict format

Return both:

- **Spec compliance:** Approved / Rejected, with each missing requirement listed.
- **Code quality:** Approved / Rejected, with severity of every finding.

Critical or Important findings block progression. Do not make code changes in this review.
