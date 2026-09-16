# Mammoth — durable context memory for coding agents

This archive contains `mammoth` and `mammoth-mcp`. Start with:

```bash
mammoth memory --project my-app recall
mammoth-mcp --project my-app
```

The MCP process is launched by your coding agent and communicates over stdio.
See [MCP setup](MCP.md) and [memory durability and limits](AGENT-MEMORY.md).
Use absolute paths in client configurations and the same local root for CLI and MCP.
The default root is `~/.mammoth/local`. No Node runtime is required for memory.

The older storage service remains in AI_coded builds as a local preview. Its
operator and release-readiness notes are included as historical material.
