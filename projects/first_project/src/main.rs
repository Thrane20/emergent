mod avoidance;
mod bounce;
mod components;
mod game;
mod generated;
mod spin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    emergent::project_host::main::<game::FirstProjectGame>(env!("CARGO_MANIFEST_DIR"))
}
