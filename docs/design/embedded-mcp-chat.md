# Embedded MCP chat

The IDE owns an MCP client and server per chat request, connected with the installed
SDK's in-memory transport. External clients retain the HTTP/stdio connection. Tool
schemas and server instructions come from MCP, rather than a second tool registry.

The client supplies the current scope, scene, authored statements, project guide and
capability catalogue. Claude Messages and OpenAI Responses adapters (also used by
Vercel AI Gateway) preserve complete
assistant/tool turns, including reasoning and image content. Tools execute sequentially;
the conversation is bounded to 24 model turns and 32 tool calls. This runs on the IDE
server, outside engine frame paths, and adds no runtime ownership or identity model.
Only durable authored refs cross the model boundary; SlotMap handles remain internal.

Each server instance reads the starting project root and rejects calls after the
linked project changes. Existing operations retain their revision checks, Rust
validation and undo journal. Completed edits are reported even if a later request
fails. A terminal `implement_project_code` handoff uses the existing staged file
workflow, only before any MCP mutation, and re-reads the project before proposing
files. This prevents two mutation mechanisms from overlapping in one conversation.

UI authoring uses the shared `list_ui_widgets`, `create_ui`, `edit_ui` and
`preview_ui` tools. UI assets participate in `find`/`list`/`describe`, so UI requests
do not need the private code handoff. The external HTTP/stdio clients receive the
same UI tools and durable refs.

Validation: local provider stand-ins exercise both wire formats and continuation;
in-memory integration tests exercise real tool discovery, resource reads, project
binding, validation errors, limits, completed edits and code handoff. Paid model
quality and native gameplay remain separate checks.

Vercel AI Gateway reuses the Responses adapter for MCP tools and the existing
structured-output path for Statements/editors. Provider settings own the Gateway
key, URL and `creator/model` ID independently of Claude/Codex. The Settings picker
loads Vercel's public catalogue through `/api/settings/ai/models`, without credentials,
and filters language models advertising tools and structured output. Manual IDs
remain usable when discovery fails or for custom models. Saving applies to subsequent
requests; an active tool loop retains its connection settings. No engine storage,
SlotMap identity, game template or frame-critical path changes are needed.
Research and validation: [Vercel AI Gateway](../platform/vercel-ai-gateway.md).
