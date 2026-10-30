//! Headless asset bridge used by the IDE; stdout is reserved for field data.
use emergent::{NoiseDefinition, NoiseGenerator, Project};
use std::{
    error::Error,
    io::{Read, Write},
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 || args[0] != "--project" {
        return Err(
            "usage: --project <root> [--asset <path>] [--candidate <file>] [--generate]".into(),
        );
    }
    let project = Project::load(&args[1])?;
    let asset = (args[2] == "--asset").then_some(&args[3]);
    if let Some(path) = asset {
        if !path.ends_with(".noise.toml") {
            return Err("Choose a .noise.toml filter asset".into());
        }
        project.asset_path(path)?;
    }
    let mut index = if asset.is_some() { 4 } else { 2 };
    let definition = if args.get(index).map(String::as_str) == Some("--candidate") {
        let file = args.get(index + 1).ok_or("Candidate file is required")?;
        index += 2;
        let mut text = String::new();
        std::fs::File::open(file)?
            .take(256 * 1024 + 1)
            .read_to_string(&mut text)?;
        NoiseDefinition::from_source(&text)?
    } else {
        NoiseDefinition::load(
            &project,
            asset.ok_or("An asset or candidate file is required")?,
        )?
    };
    let generate = args.get(index).map(String::as_str) == Some("--generate");
    if generate {
        index += 1;
    }
    if index != args.len() {
        return Err("Unexpected noise tool arguments".into());
    }
    if generate {
        let field = NoiseGenerator::new(definition.noise)?.generate()?;
        let mut output = std::io::BufWriter::new(std::io::stdout().lock());
        output.write_all(b"ENF1")?;
        for n in [field.width, field.height, field.depth] {
            output.write_all(&n.to_le_bytes())?;
        }
        for value in field.values {
            output.write_all(&value.to_le_bytes())?;
        }
        output.flush()?;
    }
    Ok(())
}
