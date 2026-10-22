use emergent::Project;

#[test]
fn checked_in_project_contract_is_valid() {
    let project = Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
    assert_eq!(project.manifest().id, "emergent.first-project");
    assert!(project.entry_scene_path().is_file());
    assert_eq!(project.enabled_outputs().count(), 1);
}
