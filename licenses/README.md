# License material provenance

`provenance.json` records unmodified reference texts from SPDX license-list-data v3.26.0 and additional upstream texts pinned to the commit recorded in the corresponding published crate's `.cargo_vcs_info.json`. SHA-256 values are checked by the Windows packaging tool. These third-party files retain their original licenses; Glint's GPL does not replace them.

Reference texts supply complete standard terms when a crate publishes only an SPDX expression or refers to terms outside its archive. They do not supply a missing author attribution or imply that SPDX authored the dependency. Packaging also preserves each vendored crate's original license/notice/REUSE files, public Cargo authors/repository metadata and source copyright headers. The source archive retains the full, unmodified Cargo-vendored dependency sources and checksums.

The generated inventory distinguishes the actual Windows normal/build dependency graph from additional source-only packages retained for Cargo resolution. An inventory and copied notices are evidence for review; they do not mark Glint's M5 release gate, other-platform support, clean-machine runtime tests or third-party asset review complete. Dependency updates require reviewing the expressions, upstream exceptions/assets and any supplemental provenance again.
