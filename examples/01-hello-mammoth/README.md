# 01 · Hello Mammoth

Put a file in, read it back out. Two minutes.

```bash
mammoth quickstart
echo "hello from mammoth" > /tmp/hello.txt

mammoth put /tmp/hello.txt /data/hello.txt
mammoth ls /data -l
mammoth cat /data/hello.txt
```

`hello.txt` is a few bytes, well under the 1 MiB `inline_threshold`, so it never
becomes a block at all — its bytes live in the metadata store. Confirm it:

```bash
mammoth stat /data/hello.txt --json | jq '{blocks, inlined}'
# { "blocks": 0, "inlined": true }
```

That is Mammoth's answer to the small-file problem. A million files like this
cost a million metadata entries and *zero* block-report entries.

Next: [02-see-your-blocks](../02-see-your-blocks/) — a file big enough to have blocks.
