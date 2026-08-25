# Benchmark suite

Full-cluster, reproducible, publishable. This is marketing collateral, so it has
to be honest: publish the harness, the hardware spec, and the raw numbers.

```bash
mammoth bench dfsio    --write --size 10GB
mammoth bench terasort --size 100GB --report bench.json
mammoth bench metadata --ops 10000000
```

**Never cherry-pick.** A public, repeatable benchmark is the best marketing asset
this project can have, and a contested one is the worst.
