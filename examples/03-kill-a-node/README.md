# 03 · Kill a node, watch it heal

The demo that explains replication better than any diagram.

```bash
cd ../../deploy/compose
docker compose up -d
```

Load some data and confirm it is healthy:

```bash
head -c 500000000 /dev/urandom > /tmp/data.bin
mammoth put /tmp/data.bin /data/data.bin
mammoth viz health
```

Now open <http://localhost:8080/distribution> in one window, and in another:

```bash
docker compose stop worker-3
mammoth viz health --live
```

```
  ● healthy (3/3)      ████████████████████████████  4,201,882   99.97%
  ◐ under-repl (2/3)   ▎                                 1,204    0.03%

  recovery queue   1,216 blocks    ▓▓▓▓▓▓▓░░░░░░░  52%   ETA 4m 12s
  recovery rate    284 blk/s · 3.1 GB/s
  cause            worker-3 went dead 12m ago
```

Reads never fail — two copies remain the whole time. Bring it back and watch the
over-replicated blocks get reclaimed:

```bash
docker compose start worker-3
mammoth viz health --live
```
