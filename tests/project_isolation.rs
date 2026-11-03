//! Engine and IDE contracts must remain independent of any particular game.
use std::{fs, path::Path};

#[test]
fn engine_workspace_contains_only_engine_and_template_not_game_projects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    let members: Vec<_> = cargo["workspace"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap())
        .collect();
    // The templates are build fixtures for new projects and plugins; games build on their own.
    assert_eq!(members, ["templates/standard", "templates/plugin"]);
}

#[test]
fn shared_engine_ide_and_smokes_do_not_refer_to_sample_game_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![
        root.join("src"),
        root.join("src/emergent-ide/src"),
        root.join("src/emergent-ide/scripts"),
    ];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                if entry.file_name() != "emergent-ide" {
                    pending.push(entry.path());
                }
            } else if entry.path().extension().is_some_and(|ext| {
                matches!(ext.to_str(), Some("rs" | "ts" | "tsx" | "mts" | "mjs"))
            }) {
                let source = fs::read_to_string(entry.path()).unwrap();
                for forbidden in [
                    "first_project",
                    "first-project",
                    "FirstProject",
                    "First Project",
                ] {
                    assert!(
                        !source.contains(forbidden),
                        "{} depends on {forbidden}",
                        entry.path().display()
                    );
                }
            }
        }
    }
}
