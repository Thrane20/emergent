# Engine-owned validation data

`editor-project` contains small, fixed assets/scenes for engine examples and IDE
smokes. Its manifest can be loaded headlessly; executable disposable projects use
`templates/standard` code plus this asset overlay, through the IDE's
`scripts/project-fixture.ts`. Fixtures are not new-project defaults.

`models/robot-arm.glb` is the frozen TRELLIS-generated model used by import,
texture/UV and model-split regression tests. Keeping it here makes those tests
independent of game content. Game asset edits do not update fixture expectations.
