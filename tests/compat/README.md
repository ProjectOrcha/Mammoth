# Hadoop compatibility

Runs a real Hadoop in Docker and asserts parity on the two things a migration
depends on:

1. **WebHDFS** responses, so existing clients keep working through `mammoth compat`.
2. The **`MD5-of-MD5-of-CRC32C` composite checksum**, so `mammoth migrate verify`
   can compare source and destination without a second full read of either.

That second one is the feature that makes people trust the migration.
