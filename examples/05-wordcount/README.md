# 05 · Word count

The "hello world" of distributed compute, on Mammoth's DAG engine.

> Requires milestone M7. Until then, this example documents the target shape.

```bash
mammoth put ./shakespeare.txt /data/shakespeare.txt

mammoth job submit wordcount \
    --input  /data/shakespeare.txt \
    --output /data/wordcount-out

mammoth job status --follow
mammoth cat /data/wordcount-out/part-00000 | head
```

The shuffle is where the time goes — an all-to-all network transfer plus a disk
sort. Watch it happen:

```bash
mammoth viz flow
```

```
  DATA MOVEMENT  ·  last 60s

  clients     ──── 2.1 GB/s ────▶  w1 w2 w4
  shuffle     ──── 890 MB/s ─────▶  cross-rack

  cross-rack traffic  4.0 GB/s / 10 GB/s link   ▓▓▓▓▓▓░░░░  40%
```

For SQL rather than hand-written map/reduce, Mammoth implements DataFusion's
`ObjectStore` trait — so `SELECT` works without a compute engine of our own.
