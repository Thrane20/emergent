use emergent::{EulerRot, Project, Quat, SceneDefinition, Transform, Vec3, World};
use serde::{Deserialize, Serialize};
use std::{error::Error, io::Read};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Values {
    translation: [f32; 3],
    rotation_degrees: [f32; 3],
    scale: [f32; 3],
}
impl From<Transform> for Values {
    fn from(t: Transform) -> Self {
        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
        Self {
            translation: t.translation.to_array(),
            rotation_degrees: [x, y, z].map(f32::to_degrees),
            scale: t.scale.to_array(),
        }
    }
}
#[derive(Serialize)]
struct Instance {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    look_direction: Option<[f32; 3]>,
    translation: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    global: Option<Values>,
}
#[derive(Serialize)]
struct Report {
    instances: Vec<Instance>,
}

pub fn report(project: &Project, path: &str) -> Result<(), Box<dyn Error>> {
    let mut world = World::new();
    let scene = SceneDefinition::load_with(project, path, &crate::components::registry())?
        .instantiate(&mut world)?;
    world.propagate_transforms();
    let mut ids: Vec<_> = scene.entities.keys().collect();
    ids.sort();
    let instances = ids
        .into_iter()
        .map(|id| {
            let global = world
                .global_transform(scene.entities[id])
                .expect("constructed scene entity exists");
            Instance {
                id: id.clone(),
                look_direction: global
                    .look_direction()
                    .map(|direction| direction.to_array()),
                translation: global.translation().to_array(),
                global: Transform::try_from_matrix(global.matrix())
                    .ok()
                    .map(Into::into),
            }
        })
        .collect();
    print!("{}", toml::to_string(&Report { instances })?);
    Ok(())
}
pub fn convert(
    project: &Project,
    path: &str,
    id: &str,
    file: &str,
    position_only: bool,
) -> Result<(), Box<dyn Error>> {
    let mut source = String::new();
    std::fs::File::open(file)?
        .take(4097)
        .read_to_string(&mut source)?;
    if source.len() > 4096 {
        return Err("global transform request exceeds 4096 bytes".into());
    }
    let values: Values = toml::from_str(&source)?;
    let mut world = World::new();
    let scene = SceneDefinition::load_with(project, path, &crate::components::registry())?
        .instantiate(&mut world)?;
    world.propagate_transforms();
    let entity = *scene
        .entities
        .get(id)
        .ok_or("scene instance does not exist")?;
    let local = if position_only {
        world
            .local_from_global_position(entity, Vec3::from_array(values.translation))
            .map_err(|error| error.to_string())?
    } else {
        let [x, y, z] = values.rotation_degrees.map(f32::to_radians);
        world
            .local_from_global(
                entity,
                Transform::new(
                    Vec3::from_array(values.translation),
                    Quat::from_euler(EulerRot::XYZ, x, y, z),
                    Vec3::from_array(values.scale),
                ),
            )
            .map_err(|error| error.to_string())?
    };
    print!("{}", toml::to_string(&Values::from(local))?);
    Ok(())
}
