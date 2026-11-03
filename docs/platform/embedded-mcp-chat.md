# Embedded MCP chat — local validation

2026-11-01, macOS development workspace. This is local integration evidence; no paid
provider request, browser interaction or native gameplay smoke was performed.

The IDE project chat now creates a per-request MCP client/server over in-memory
transport. Claude Messages and OpenAI Responses use the active AI settings. The
server supplies the tools and instructions, and guide/capability resources accompany
the scoped request. Structured editor AI and Statements retain their existing path.

Checks from `src/emergent-ide`:

- `npx tsx --conditions=react-server --test tests/ai-chat.test.ts tests/ai-tool-conversation.test.ts tests/chat-implementation.test.ts tests/mcp-chat.test.ts tests/mcp-server.test.ts tests/statement-ai-claude.test.ts tests/statements.test.ts tests/ui-assets.test.ts`: **16 passed**.
- `npx tsc --noEmit`: passed.
- Targeted ESLint on the five touched/new library modules and two new test files:
  passed.
- `npm run build`: passed, including compilation, TypeScript and all route generation.
- Final source also passed `EMERGENT_NEXT_DIST_DIR=.next-mcp-chat-build npm run build`;
  temporary build output and its generated TypeScript include entries were removed.

Provider stand-ins verify tool schema submission, full assistant continuation,
tool-result IDs, error results, image results, OpenAI encrypted reasoning retention,
incomplete responses and no automatic network retry. MCP integration uses real
discovery/resources and exercises malformed edits, project-switch rejection,
24-turn/32-call bounds, dry run, a journalled media-ID mutation followed by provider
failure, and refusal to mix completed MCP edits with a code handoff. `/api/chat` is
also exercised end to end against a local Responses stand-in that calls `find`.
Existing tests still exercise actual isolated Cargo/scene validation, conflict
rejection, rollback and UI asset validation for file changes.

Limits: tool decisions and output quality with live Claude/OpenAI remain unverified.
Each MCP edit commits independently; failure later in the conversation preserves
earlier edits and reports their change IDs. New Rust code uses the existing staged
workflow. No engine frame-time or gameplay performance claim is made.
