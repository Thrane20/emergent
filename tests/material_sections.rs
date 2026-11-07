#![cfg(feature = "graphics")]
//! Material sections draw their index range with their own material, the rest with the mesh's.
use emergent::{
    Color, GeometryInstance, GeometryRenderer, Mat4, MaterialDefinition, MaterialSection,
    MaterialShading, Mesh, MeshVertex, Vec3,
};

#[test]
fn each_section_renders_with_its_own_material() {
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(64, 64)).unwrap();
    let v = |x: f32, y: f32| {
        MeshVertex::with_normal(Vec3::new(x, y, 0.), Vec3::Z, Color::WHITE).unwrap()
    };
    // Left quad (indices 0..6) uses the mesh material; right quad (6..12) is a section.
    let quad = Mesh::new(
        vec![
            v(-1., -1.),
            v(0., -1.),
            v(0., 1.),
            v(-1., 1.),
            v(0., -1.),
            v(1., -1.),
            v(1., 1.),
            v(0., 1.),
        ],
        vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
    )
    .unwrap();
    let unlit = |albedo| {
        MaterialDefinition {
            shading: MaterialShading::Unlit,
            albedo: Some(albedo),
            ..Default::default()
        }
        .constant()
        .unwrap()
    };
    let sections = [MaterialSection {
        indices: 6..12,
        material: unlit([0., 0., 1.]),
    }];
    let geometry = renderer
        .insert_with_sections(&quad.clone().into(), &unlit([1., 0., 0.]), &sections)
        .unwrap();
    renderer
        .set_camera(
            Vec3::new(0., 0., 3.),
            Vec3::ZERO,
            Vec3::Y,
            emergent::CameraProjection::isometric(2., 0.1, 10.).unwrap(),
        )
        .unwrap();
    let frame = renderer
        .render_lit_rgba(&[GeometryInstance::new(geometry, Mat4::IDENTITY)], &[])
        .unwrap();
    let pixel = |x: u32| {
        let i = ((frame.height / 2 * frame.width + x) * 4) as usize;
        [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
    };
    let (left, right) = (pixel(frame.width / 4), pixel(frame.width * 3 / 4));
    assert!(
        left[0] > 200 && left[2] < 50,
        "left uses mesh material: {left:?}"
    );
    assert!(
        right[2] > 200 && right[0] < 50,
        "right uses section material: {right:?}"
    );
    let bad = [MaterialSection {
        indices: 4..12,
        material: unlit([0., 0., 1.]),
    }];
    assert!(renderer
        .insert_with_sections(&quad.into(), &unlit([1., 0., 0.]), &bad)
        .is_err());
}
