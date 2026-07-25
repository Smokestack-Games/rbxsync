# Changelog

All notable changes to RbxSync are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] - unreleased

This release replaces the per-instance multi-file rbxjson format with a single
hidden, read-only `datamodel.rbxjson` context document at the project root, and
keeps scripts as standalone `.luau` files on disk.

### Added

- Single `datamodel.rbxjson` context document: the flat instance array is assembled
  into a nested context tree and written once per extract, with scripts carrying a
  `sourcePath` pointer instead of inline source.
- CLI `extract --from-file` builds the baseline snapshot from a saved place file and
  writes one `datamodel.rbxjson`, adopting existing scripts.
- Live Studio deltas (create/modify/delete/rename) are patched into the context tree
  and relayed into `datamodel.rbxjson` via a debounced flush, with re-buffering when
  a flush fails.
- rbx-dom stack upgraded to parse modern place files.

### Changed

- Server extract path migrated to the context-file model: instance state now lives
  only in `datamodel.rbxjson`, and read/sync handlers resolve against the context
  document.
- VS Code treats `datamodel.rbxjson` as a hidden, read-only context file and includes
  script-less services in the generated `default.project.json`.
- Core plans script writes as standalone files and preserves duplicate-named siblings
  in the context document.

### Removed

- Retired the per-instance `.rbxjson` + `_meta.rbxjson` multi-file format, including
  the non-script `.rbxjson` readers and the file-watcher push path on the server.
- Retired the standalone rbxjson language server.
- Removed the per-instance VS Code decorations, tree view, and `openMetadata` command.
- Removed the flat-format schema pipeline and the rbxjson schema update CI workflow.
