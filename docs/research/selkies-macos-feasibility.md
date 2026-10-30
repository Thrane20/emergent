# Selkies feasibility gate for local Mac development

Date: 2026-10-24. Outcome: **do not adopt Selkies for the native macOS preview**.
Stop at the compatibility gate, as requested, before installing a streaming stack
or porting capture/input infrastructure.

## Checks performed

| Check | Observed result |
| --- | --- |
| `uname -srm` | Darwin 27.0.0 arm64 |
| `command -v selkies` | Not installed |
| `docker context show` | `desktop-linux` |
| `docker info --format '{{.OSType}}/{{.Architecture}} {{.OperatingSystem}}'` | `linux/aarch64 Docker Desktop` |
| PyPI metadata for `selkies` | Version 2.1.0 requires `pixelflux==2.2.1` and `pcmflux==2.2.0` |
| PyPI metadata for required `pixelflux` | Version 2.2.1 publishes 28 Linux wheels, zero macOS/Darwin wheels, and no source distribution on PyPI |
| Upstream host documentation | Linux X11/Wayland capture; macOS and Windows server support are planned |
| Docker Desktop GPU documentation | The documented `--gpus` passthrough path is Windows/WSL2, not a native Mac Metal capture/encoding bridge |

The metadata checks used the public PyPI JSON endpoints, downloaded to temporary
files and parsed locally. No Python packages were installed, no container images
were pulled, and no Selkies service was started. No private project data or keys
were sent to a service. Existing IDE and game processes were left running.

## Decision and limits of the evidence

Selkies is a Linux-hosted desktop/session streaming system. Its browser client can
run on a Mac, but the host capture/input path is the issue here. Installing the
Python orchestration package would not supply a supported native macOS capture
backend. Building a new backend would turn this into a platform port.

The available Docker daemon is a local Linux environment. It could be used for a
separate Linux-rendered experiment, potentially with software rendering/encoding;
that would not establish a native Metal development path or representative engine
performance. No container graphics/encoding benchmark was run. No remote Linux
GPU worker was configured or tested during this gate.

This is a **compatibility rejection for the current local Mac workflow**, not a
runtime benchmark failure or a rejection of Selkies for a future Linux GPU worker.
Reopen it if a Linux worker becomes the intended host, or upstream adds supported
macOS hosting.

## Direction to retain

Keep the persistent inspection-session design: construct once, retain the SlotMap
world and renderer resources, run tool input/rendering without advancing gameplay
ticks, and synchronize selected instance IDs/transforms with the web inspector.
Transport and encoding should remain outside core world ownership and gameplay
hot paths. Do not turn repeated PNG subprocess captures into the production loop.

The next candidate to evaluate is a native Metal inspection process with a local
structured command channel and bounded browser frame stream. Prove persistent
scene lifetime and live inspector edits first, measure readback/encoding/input
latency, and choose the production streaming path from that evidence. This
alternative was not implemented at this compatibility gate. The subsequent
[SP-2 native prototype](../platform/persistent-inspection-macos.md) now proves
persistent lifetime and bounded PNG delivery; the production transport is still open.

## Sources checked

- [Selkies architecture and host platforms](https://github.com/selkies-project/selkies/blob/main/docs/design.md)
- [Selkies package metadata](https://pypi.org/pypi/selkies/json)
- [pixelflux package metadata](https://pypi.org/pypi/pixelflux/json)
- [pixelflux implementation and distributions](https://github.com/selkies-project/pixelflux)
- [Docker Desktop GPU support](https://docs.docker.com/desktop/features/gpu/)
