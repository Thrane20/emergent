#![cfg(feature = "graphics")]
//! UV-mapped material images sample by mesh texture coordinates, not by position.
use emergent::{
    Color, GeometryInstance, GeometryRenderer, Mat4, Material, MaterialDefinition, MaterialShading,
    Mesh, MeshVertex, TextureImage, Vec2, Vec3,
};

fn red_blue_checker() -> TextureImage {
    let image = image::RgbaImage::from_fn(64, 64, |x, y| {
        if (x < 32) == (y < 32) {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 255, 255])
        }
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    TextureImage::from_bytes(bytes.get_ref()).unwrap()
}

#[test]
fn texture_coordinates_choose_the_texel_a_pixel_shows() {
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(64, 64)).unwrap();
    // U runs right-to-left, so the texture's red half lands on the quad's right side.
    let corner = |x: f32, y: f32, u: f32, v: f32| {
        MeshVertex::with_normal(Vec3::new(x, y, 0.), Vec3::Z, Color::WHITE)
            .unwrap()
            .with_uv(Vec2::new(u, v))
            .unwrap()
    };
    let quad = Mesh::new(
        vec![
            corner(-1., -1., 1., 1.),
            corner(1., -1., 0., 1.),
            corner(1., 1., 0., 0.),
            corner(-1., 1., 1., 0.),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
    .unwrap();
    let material = Material::with_uv_image(
        MaterialDefinition {
            shading: MaterialShading::Unlit,
            texture_tiling: [2.0, 2.0],
            ..Default::default()
        },
        red_blue_checker(),
    )
    .unwrap();
    assert!(material.uv_mapped());
    let geometry = renderer
        .insert_with_material(&quad.into(), &material)
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
    let pixel = |x: u32, y: u32| {
        let i = ((y * frame.width + x) * 4) as usize;
        [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
    };
    let at = |x: u32, y: u32| pixel(frame.width * x / 8, frame.height * y / 8);
    let (origin, x_alternate, y_alternate) = (at(1, 1), at(3, 1), at(1, 3));
    assert_eq!(origin, at(5, 1), "X tiling repeats after half the quad");
    assert_eq!(origin, at(1, 5), "Y tiling repeats after half the quad");
    assert_ne!(origin, x_alternate, "X tiling reaches the other texel");
    assert_ne!(origin, y_alternate, "Y tiling reaches the other texel");
}
