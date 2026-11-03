use emergent::{ui::UiDefinition, ui_surface::rasterize, UiSurface};
use std::sync::Arc;
fn surface(screen: bool) -> UiSurface {
    let source="schema_version=1\nid='hud'\ncanvas=[64,64]\n[[nodes]]\nid='root'\ntype='panel'\nfill=[1,0,0]\n[[nodes]]\nid='label'\nparent='root'\ntype='label'\ntext='UI 123'\ncolour=[0,1,0]\nw='auto'\nh='auto'\n";
    UiSurface {
        definition: Arc::new(UiDefinition::from_source("hud", source).unwrap()),
        screen,
        width: if screen { 32.0 } else { 2.0 },
        height: if screen { 32.0 } else { 2.0 },
        x: 8.0,
        y: 8.0,
    }
}
#[test]
fn prepared_ui_has_real_glyphs_transparency_and_bounded_images() {
    let ui = surface(true);
    let image = rasterize(&ui.definition).unwrap();
    assert_eq!(image.width(), 64);
    assert!(image
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[1] > 200 && p[0] == 0));
    assert!(image.rgba().as_chunks::<4>().0.contains(&[255, 0, 0, 255]));
    let (_, material) = ui.prepare().unwrap();
    assert!(material.uv_mapped());
    let mut bad = ui.clone();
    bad.width = -1.0;
    assert!(bad.validate().is_err());
    let source = "schema_version=1\nid='t'\ncanvas=[8,8]\n[[nodes]]\nid='r'\ntype='grid'";
    let transparent = UiDefinition::from_source("t", source).unwrap();
    assert!(rasterize(&transparent)
        .unwrap()
        .rgba()
        .iter()
        .all(|p| *p == 0));
}
#[cfg(feature = "graphics")]
#[test]
fn overlays_ignore_camera_and_world_panels_are_depth_tested() {
    use emergent::{Color, Geometry, GeometryInstance, GeometryRenderer, Mat4, Primitive, Vec3};
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(96, 96)).unwrap();
    let overlay = renderer.insert_ui_surface(&surface(true)).unwrap();
    let instances = [GeometryInstance::new(overlay, Mat4::IDENTITY)];
    let a = renderer.render_lit_rgba(&instances, &[]).unwrap();
    renderer
        .set_view(Vec3::new(4., 7., 9.), Vec3::ZERO, Vec3::Y)
        .unwrap();
    let b = renderer.render_lit_rgba(&instances, &[]).unwrap();
    assert_eq!(a.rgba, b.rgba, "overlay must ignore camera movement");
    let pixel = |f: &emergent::RenderedFrame, x: usize, y: usize| {
        let at = (y * f.width as usize + x) * 4;
        f.rgba[at..at + 4].to_vec()
    };
    assert!(
        pixel(&a, 10, 10)[0] > 200,
        "overlay should render red at its pixel position"
    );
    assert!(
        pixel(&a, 80, 80)[0] < 200,
        "overlay must not cover the whole viewport when sized"
    );
    renderer
        .set_view(Vec3::new(0., 0., 5.), Vec3::ZERO, Vec3::Y)
        .unwrap();
    let world = renderer.insert_ui_surface(&surface(false)).unwrap();
    let world_instances = [GeometryInstance::new(world, Mat4::IDENTITY)];
    let visible = renderer.render_lit_rgba(&world_instances, &[]).unwrap();
    assert!(pixel(&visible, 48, 48)[0] > 200);
    let cube = renderer.insert(&Geometry::from(Primitive::cube(0.5, Color::BLUE).unwrap()));
    let occluded = renderer
        .render_lit_rgba(
            &[
                world_instances[0],
                GeometryInstance::new(cube, Mat4::from_translation(Vec3::new(0., 0., 1.))),
            ],
            &[],
        )
        .unwrap();
    assert!(
        pixel(&occluded, 48, 48)[2] > pixel(&occluded, 48, 48)[0] + 60,
        "world panel must be occluded by foreground geometry: {:?}",
        pixel(&occluded, 48, 48)
    );
    let mut front = surface(true);
    front.x = 32.0;
    front.y = 32.0;
    let top = renderer.insert_ui_surface(&front).unwrap();
    let composed = renderer
        .render_lit_rgba(
            &[
                GeometryInstance::new(cube, Mat4::from_translation(Vec3::new(0., 0., 1.))),
                GeometryInstance::new(top, Mat4::IDENTITY),
            ],
            &[],
        )
        .unwrap();
    assert!(
        pixel(&composed, 48, 48)[0] > 200 && pixel(&composed, 48, 48)[2] < 20,
        "overlay must composite over foreground geometry"
    );
    renderer.resize(winit::dpi::PhysicalSize::new(128, 96));
    let resized = renderer
        .render_lit_rgba(&[GeometryInstance::new(top, Mat4::IDENTITY)], &[])
        .unwrap();
    assert_eq!(resized.width, 128);
    assert!(pixel(&resized, 48, 48)[0] > 200);
    assert!(renderer.remove(overlay));
    assert!(!renderer.contains(overlay));
    assert!(renderer.render_lit_rgba(&instances, &[]).is_err());
}
