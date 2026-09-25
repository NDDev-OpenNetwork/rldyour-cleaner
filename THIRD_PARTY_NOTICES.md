# Third-party notices

This crate depends on the following Rust libraries:

| Crate | License | Used for |
|---|---|---|
| clap | MIT OR Apache-2.0 | command-line parsing |
| serde | MIT OR Apache-2.0 | policy/report (de)serialization |
| serde_json | MIT OR Apache-2.0 | report JSON |
| toml | MIT OR Apache-2.0 | policy file format |
| walkdir | Unlicense OR MIT | directory traversal |
| fs2 | MIT OR Apache-2.0 | probing cargo's `.cargo-lock` |
| libc | MIT OR Apache-2.0 | `statvfs` disk-pressure probe |

Dev-only:

| Crate | License | Used for |
|---|---|---|
| filetime | MIT OR Apache-2.0 | fixture mtimes in tests |
