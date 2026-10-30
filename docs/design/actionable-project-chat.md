# Actionable project chat

Build with AI uses the existing statement proposal/parser, inspected project and
foundation context, isolated Cargo/scene checks and conflict-checked journalled
publication. No new engine identity model or execution tool is introduced.

Game, Scene and Object requests retain their scope; the active scene is supplied
separately so project-level creation targets the scene being viewed. New authored
objects receive supplied UUIDs and separate display names. World-origin creation
uses a root instance, preserving existing objects and parent-local transforms.

Questions return a summary with no replacements. Edit requests return complete
bounded replacement files. Allowed writes remain project Rust and asset TOML;
engine code, manifests, dependencies and arbitrary command execution are excluded.
Chat and statement generation share their existing in-process implementation lock.
Before publication, validate candidates away from live files and recheck project
binding and inspected sources. Failed generation, compilation, scene loading or
conflicts must not publish a partial change. Existing journal rollback applies.

After publication, return actual changed paths; refresh project/inspector state
and restart paused inspection when Scene is visible. Block conflicting local
inspector/navigation actions while chat is working. Validation uses a temporary
project, mocked structured AI output and actual Cargo/scene validation, plus UI
request/refresh checks; no paid provider request is required for regression tests.

Reuse evidence distinguishes inspected TOML asset filenames from Rust identifiers.
Prompts include exact inspected-path/citation examples. One targeted correction
attempt is allowed for citation rejection, before any file preparation/publication;
a second rejection stops with a path/symbol diagnostic. Other failures are not
automatically retried. Both chat and statement implementation use this boundary.
