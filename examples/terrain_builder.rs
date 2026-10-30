//! Headless terrain tool. ETF1 contains a count followed by timed dense height fields.
use emergent::TerrainSettings;
use std::{
    error::Error,
    io::{Read, Write},
    time::Instant,
};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let mut source = None;
    let mut compare = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--settings" => source = Some(args.next().ok_or("--settings needs a file")?),
            "--compare" => compare = true,
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let mut text = String::new();
    std::fs::File::open(source.ok_or("Use --settings <terrain.toml> [--compare]")?)?
        .take(16385)
        .read_to_string(&mut text)?;
    if text.len() > 16384 {
        return Err("Terrain settings exceed 16 KiB".into());
    }
    let settings: TerrainSettings = toml::from_str(&text)?;
    settings.validate()?;
    let recipes = if compare {
        settings.comparison().to_vec()
    } else {
        vec![settings]
    };
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    out.write_all(b"ETF1")?;
    out.write_all(&(recipes.len() as u32).to_le_bytes())?;
    for recipe in recipes {
        let start = Instant::now();
        let field = recipe.generate()?;
        let nanos = start.elapsed().as_nanos() as u64;
        out.write_all(&nanos.to_le_bytes())?;
        out.write_all(&recipe.resolution.to_le_bytes())?;
        for value in field.values() {
            out.write_all(&value.to_le_bytes())?;
        }
    }
    out.flush()?;
    Ok(())
}
