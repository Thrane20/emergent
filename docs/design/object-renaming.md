# Object names and editable asset source

An optional name on a scene instance overrides its entity asset's shared display
name. The engine resolves that name during scene construction; durable instance
IDs, asset paths, parenting keys and runtime SlotMap identity stay unchanged.
Older scenes without the field retain their existing names. Limit local names to
1–64 printable Unicode characters, trimming surrounding whitespace in the IDE.

Reuse the existing guarded TOML editor for entity Source views. Shared source edits
validate and persist through the current TOML route. Enter on a selected graph row
opens a local inline name editor; Enter/blur saves, Escape cancels. Local edits use
the scene revision and existing staged scene validation/publication boundary,
preserve comments and unrelated tables, and update inspector/graph labels.
No new renderer resources or frame-loop work are needed.
