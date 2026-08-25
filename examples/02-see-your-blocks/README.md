# 02 · See your blocks

The moment this stops feeling like homework.

```bash
# 350 MB of nothing in particular — three 128 MB blocks, the last one partial
head -c 350000000 /dev/urandom > /tmp/big.bin

mammoth put /tmp/big.bin /data/big.bin
mammoth viz blocks /data/big.bin
```

```
  /data/big.bin   350 MB · 3 blocks · replication 3

           w1    w2    w3
  blk 1    ●     ●     ●
  blk 2    ·     ●     ●
  blk 3    ●     ·     ●

  ● primary   ◐ replica   ✕ corrupt   · absent
```

Then look at the cluster as a whole, and at what is eating the space:

```bash
mammoth viz cluster
mammoth viz treemap / --depth 2
mammoth viz topology
```

Every one of these has a `--json` form, so the same views script cleanly:

```bash
mammoth viz blocks /data/big.bin --json | jq '.blocks[] | {index, replicas: [.replicas[].node]}'
```
