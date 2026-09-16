# Branches

Both `main` and `AI_coded` provide Mammoth's durable coding-agent memory: the same
memory crate, MCP server, CLI workflow, product homepage, and memory documentation.
Use either branch for the memory quickstart. Memory changes should be validated
on both; generated legacy CLI references can differ because the underlying command
trees differ.

`main` still contains the manual storage-engine teaching scaffold. `AI_coded`
retains the implemented local storage service and its supporting dashboard and
benchmarks. Those historical differences do not gate the agent-memory workflow.
Do not replace one branch wholesale with the other when synchronizing memory work.
