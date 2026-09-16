# Kubernetes deployment is not implemented

The placeholder chart was removed from `AI_coded`: it created no workloads, yet
its notes implied a working distributed cluster. There is no supported Helm
installation or published container image for this branch.

Use the single-host [Docker Compose configuration](../compose/docker-compose.yml)
for an isolated local deployment, or the [operator runbook](../../docs/OPERATIONS.md)
for a native service. Compose's worker directories share one disk and one failure
domain. It does not implement Kubernetes HA, worker networking or authentication.

A future chart needs actual master/worker services, security, persistent-volume
recovery, probes, resource limits, upgrades and failure tests before installation
instructions are published. See [release gates](../../docs/RELEASE-READINESS.md).
