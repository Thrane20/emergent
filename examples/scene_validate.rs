//! Validate staged assets without building a game executable into its live output slot.
use emergent::{Project, SceneDefinition};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--project") {
        return Err("Use --project <root> --validate-scenes <scene...>".into());
    }
    let project = Project::load(args.next().ok_or("Missing project root")?)?;
    if args.next().as_deref() != Some("--validate-scenes") {
        return Err("Missing --validate-scenes".into());
    }
    let scenes: Vec<_> = args.collect();
    if scenes.is_empty() || scenes.len() > 127 {
        return Err("Validate 1–127 scenes".into());
    }
    for scene in scenes {
        SceneDefinition::load(&project, &scene)?;
    }
    Ok(())
}
