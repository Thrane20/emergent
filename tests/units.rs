//! Metre contract across prepared geometry, instance scale and hierarchy transforms.
use emergent::{Color, Mat4, Mesh, Primitive, TerrainSettings, Transform, Vec3, World};

fn size(mesh: &Mesh, matrix: Mat4) -> Vec3 {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in mesh.vertices() {
        let position = matrix.transform_point3(vertex.position());
        min = min.min(position);
        max = max.max(position);
    }
    max - min
}

#[test]
fn unit_cube_at_half_scale_has_fifty_centimetre_sides() {
    let cube = Primitive::cube(0.5, Color::WHITE)
        .unwrap()
        .to_mesh()
        .unwrap();
    assert_eq!(size(&cube, Mat4::IDENTITY), Vec3::ONE);
    assert_eq!(
        size(&cube, Mat4::from_scale(Vec3::splat(0.5))),
        Vec3::splat(0.5)
    );
    let mut world = World::new();
    let parent = world.spawn("parent");
    let child = world.spawn_child(parent, "cube").unwrap();
    world
        .set_local_transform(parent, Transform::from_scale(Vec3::new(2., 3., 4.)))
        .unwrap();
    world
        .set_local_transform(child, Transform::from_scale(Vec3::splat(0.5)))
        .unwrap();
    world.propagate_transforms();
    assert_eq!(
        size(&cube, world.global_transform(child).unwrap().matrix()),
        Vec3::new(1., 1.5, 2.)
    );
}

#[test]
fn scale_one_keeps_authored_size_and_half_extents_remain_half_sizes() {
    let ground = Primitive::plane(6., 6., Color::WHITE)
        .unwrap()
        .to_mesh()
        .unwrap();
    assert_eq!(size(&ground, Mat4::IDENTITY), Vec3::new(12., 0., 12.));
    assert!(size(&ground, Mat4::from_scale(Vec3::new(1.8, 1., 1.8)))
        .abs_diff_eq(Vec3::new(21.6, 0., 21.6), 0.00001));
    let pyramid = Primitive::pyramid(0.5, 1., Color::WHITE)
        .unwrap()
        .to_mesh()
        .unwrap();
    assert_eq!(size(&pyramid, Mat4::IDENTITY), Vec3::ONE);
}

#[test]
fn terrain_size_one_thousand_is_one_kilometre_at_unit_scale() {
    let field = TerrainSettings {
        size: 1000.,
        height: 0.,
        resolution: 32,
        ..Default::default()
    }
    .generate()
    .unwrap();
    let mesh = field.mesh(Color::WHITE).unwrap();
    assert_eq!(size(&mesh, Mat4::IDENTITY), Vec3::new(1000., 0., 1000.));
    assert_eq!(
        size(&mesh, Mat4::from_scale(Vec3::splat(0.5))),
        Vec3::new(500., 0., 500.)
    );
}
