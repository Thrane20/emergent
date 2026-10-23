# IDE statements

## Ownership and identity

Statements are durable, project-authored intent owned by the game project and stored in `emergent.statements.json` beside `emergent.toml`. Each statement has its own durable UUID and targets a stable subject reference such as `file:assets/entities/vehicle.entity.toml`. Future entity subjects use the authored source path and durable entity ID. Runtime SlotMap and ECS handles never enter this file.

## Data flow

The IDE resolves the current selection to a subject, reads only that subject's statements, and sends validated mutations through a server route. Writes replace the complete versioned document atomically. AI evaluation receives the live project graph, selected source content, and one statement, then returns structured implementation status, confidence, and evidence. Evaluation may update assessment fields but never statement text or game code.

## Performance and validation

Statement I/O is project-mode work, outside the game loop. The initial scale target is hundreds of short statements per project; a single JSON document keeps inspection simple and reviewable. Validate subject references against the current project graph, serialize concurrent writes, bound text and evidence, and test create/read/evaluate flows without using runtime identities.
