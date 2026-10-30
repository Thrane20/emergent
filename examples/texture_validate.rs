//! Native image validation before publishing an imported project texture.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("Supply a PNG/JPEG file")?;
    if args.next().is_some() {
        return Err("Supply one PNG/JPEG file".into());
    }
    let image = emergent::TextureImage::load(path)?;
    println!("{} {}", image.width(), image.height());
    Ok(())
}
