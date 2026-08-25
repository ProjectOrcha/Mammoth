# Fuzzing

```bash
cargo install cargo-fuzz
cargo fuzz run parse_s3_request
```

Targets should cover anything that parses bytes from the network or from disk:
the S3 request parser, SigV4 signature parsing, the block `.meta` header, the
RPC frame decoder, and the config loader.
