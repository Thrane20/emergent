# Object Creator implementation brief

Object Creator is an IDE authoring service. The Next.js server owns fal credentials,
job journals and downloaded candidates; a game project owns only accepted asset
bundles. No Rust engine source or runtime identity contract changes. Durable UUIDs
identify jobs and assets; a future model loader will create fresh SlotMap handles.

The server uploads one PNG/JPEG/WebP image (maximum 10 MiB), submits the fixed
`fal-ai/trellis` endpoint once, and persists its request ID. Poll requests resume
queue observation and download completed GLB output (maximum 100 MiB). No provider
URL or credential is needed by the preview browser. Polling uses no game-loop work.
Jobs are partitioned by a hash of project ID and canonical root under the existing
IDE data directory. This first local IDE supports one server process; per-job
serialization and filesystem publication locks protect concurrent requests.

Submission intent is journaled before the paid call. A timeout or restart after
intent but before recording its request ID becomes `uncertain`; it is never
automatically resubmitted. Transport failures during observation retain the known
request ID and can be retried. A browser-generated job UUID makes duplicate POSTs
idempotent. Jobs progress only while polled, and resume when the view is reopened.

The browser lazily registers Model Viewer, renders a locally served GLB, and owns
orbit/zoom state independently of the asset. The server checks GLB structure and
embedded resources before save; successful viewer loading is still a separate
compatibility check. Remote downloads accept HTTPS fal media hosts only, with
redirects disabled. Asset routes resolve real paths beneath declared project roots
or the job cache; binary size limits are applied before buffering files.

Save stages the GLB, original reference, and versioned descriptor in an IDE-owned
hidden staging directory on the project's filesystem, then renames the complete
bundle into a validated asset root. Existing bundles are never replaced. A lock
serializes publication; the descriptor's job ID supports recovery after a crash
between bundle publication and journal update. The descriptor records project-relative
references, a durable asset UUID, provider/request/settings/seed/timestamps, and a
null model revision when unavailable. Saved descriptors provide explicit graph edges.

Initial scale is one developer and a small local candidate history, returned as the
latest 100 jobs. Network waits and disk preparation occur in request handlers, with
bounded per-request inputs and downloads. No frame-time performance claim is made.

Validation covers journal recovery, duplicate submission/save, transient and terminal
provider errors, invalid GLBs, bounds, symlinks/traversal, collision safety, staged
publication, project graph edges, and credential masking. Mocked tests plus lint,
TypeScript and production build establish local correctness. A funded fal key and
a real generated GLB are required for live compatibility acceptance; absent that
evidence, OC-1.1 and the full epic remain open.
