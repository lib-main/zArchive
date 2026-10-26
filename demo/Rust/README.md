# zarchive-rust demo

An application depends on the published crate:

```sh
cargo add zarchive-rust
```

This folder is that application. From here:

```sh
cargo run
```

The native libraries ship inside the `zarchive-rust` crate. The build places the set for this platform next to this program. It calls `zarchive_rust` for the format table, identify, zip create/list/read/verify/extract, a nested zip, 7z/rar/gzip/bzip2/za create, and a Windows self-extract trailer. Files are written to `out` and replaced on each run.
