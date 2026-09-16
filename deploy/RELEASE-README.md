# Mammoth local preview

This archive contains the native Mammoth binary with its dashboard embedded.
It runs a durable filesystem on one host. Authentication, TLS, separate workers,
high availability and distributed compute remain unfinished. Keep listeners on
loopback and use a separate data directory.

From this directory (use `mammoth.exe` on Windows):

```bash
./mammoth --config mammoth.toml config validate
./mammoth --config mammoth.toml --local-root /path/to/store serve --role all
```

Open http://127.0.0.1:8080 for the dashboard. From a second terminal:

```bash
./mammoth --local-root /path/to/store status
./mammoth --local-root /path/to/store doctor
./mammoth --local-root /path/to/store stop --timeout 60
```

Replace `/path/to/store` with your chosen local directory. Read `OPERATIONS.md`
for complete backup/restore and recovery instructions, and `RELEASE-READINESS.md`
for the outstanding production gates. `IMPLEMENTATION-STATUS.md` records tested
behavior. Links within those reference notes point to the source repository.

`BUILD-INFO.json` records the binary SHA256, package source revision and whether
the checkout had local changes. Verify the companion `.tar.gz.sha256` against the
archive before extraction. Checksums detect changes; they are not signatures.
The package source record assumes packaging immediately after the release build.

[Source and guides](https://github.com/ProjectOrcha/Mammoth/tree/AI_coded) ·
[Measured benchmark snapshot](https://github.com/ProjectOrcha/Mammoth/blob/AI_coded/docs/BENCHMARKS.md)

Licensed under Apache-2.0 OR MIT; both license texts are included.
