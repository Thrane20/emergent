//! Native point-shadow coverage in all directions, with independent object options.
use emergent::{
    Color, Geometry, GeometryInstance, GeometryRenderer, LightInstance, LightSource, Mat4,
    MaterialDefinition, Primitive, Quat, ShadowCasting, Vec3,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(320, 240))?;
    let cube = Geometry::from(Primitive::cube(0.55, Color::CYAN)?);
    let floor = Geometry::from(Primitive::plane(4.0, 4.0, Color::WHITE)?);
    let point =
        LightSource::point(Color::WHITE, 2.0, 15.0)?.with_shadow_casting(ShadowCasting::Enabled)?;
    let ambient = LightInstance::new(LightSource::ambient(Color::WHITE, 0.1)?, Mat4::IDENTITY);
    let mut capture = |rotation: Quat,
                       cast: bool,
                       receive: bool,
                       lights: &[LightInstance]|
     -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        renderer.set_view(
            rotation * Vec3::new(6.0, 3.0, 8.0),
            rotation * Vec3::new(0.0, -2.0, 0.0),
            rotation * Vec3::Y,
        )?;
        let caster = renderer.insert_with_material(
            &cube,
            &MaterialDefinition {
                cast_shadows: cast,
                receive_shadows: false,
                ..Default::default()
            }
            .constant()?,
        )?;
        let receiver = renderer.insert_with_material(
            &floor,
            &MaterialDefinition {
                cast_shadows: false,
                receive_shadows: receive,
                ..Default::default()
            }
            .constant()?,
        )?;
        let model = Mat4::from_quat(rotation);
        let instances = [
            GeometryInstance::new(
                caster,
                model * Mat4::from_translation(Vec3::new(0.0, -1.4, 0.0)),
            ),
            GeometryInstance::new(
                receiver,
                model * Mat4::from_translation(Vec3::new(0.0, -3.0, 0.0)),
            ),
        ];
        let result = renderer
            .render_lit_rgba(&instances, lights)
            .map(|frame| frame.rgba);
        renderer.remove(caster);
        renderer.remove(receiver);
        Ok(result?)
    };
    let changed = |a: &[u8], b: &[u8]| a.iter().zip(b).filter(|(a, b)| a != b).count();
    for direction in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        let q = Quat::from_rotation_arc(Vec3::NEG_Y, direction);
        let lights = [ambient, LightInstance::new(point, Mat4::IDENTITY)];
        let shadow = capture(q, true, true, &lights)?;
        let no_cast = capture(q, false, true, &lights)?;
        let no_receive = capture(q, true, false, &lights)?;
        let delta = changed(&shadow, &no_cast);
        assert!(
            delta > 100,
            "missing point shadow for {direction:?}: {delta}"
        );
        assert_eq!(
            no_cast, no_receive,
            "point cast/receive mismatch for {direction:?}"
        );
        println!("{direction:?}: {delta} changed bytes");
    }
    let q = Quat::IDENTITY;
    for position in [Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)] {
        let lights = [
            ambient,
            LightInstance::new(point, Mat4::from_translation(position)),
        ];
        assert!(
            changed(
                &capture(q, true, true, &lights)?,
                &capture(q, false, true, &lights)?
            ) > 100,
            "moving point light must update shadow views"
        );
    }
    let second_model = Mat4::from_translation(Vec3::new(-2.0, 0.0, 0.0));
    let off = point.with_shadow_casting(ShadowCasting::Disabled)?;
    let both = [
        ambient,
        LightInstance::new(point, Mat4::IDENTITY),
        LightInstance::new(point, second_model),
    ];
    let both_image = capture(q, true, true, &both)?;
    for lights in [
        [
            ambient,
            LightInstance::new(off, Mat4::IDENTITY),
            LightInstance::new(point, second_model),
        ],
        [
            ambient,
            LightInstance::new(point, Mat4::IDENTITY),
            LightInstance::new(off, second_model),
        ],
    ] {
        assert!(
            changed(&both_image, &capture(q, true, true, &lights)?) > 100,
            "both point shadow slots must contribute"
        );
    }
    let small =
        LightSource::point(Color::WHITE, 2.0, 0.5)?.with_shadow_casting(ShadowCasting::Enabled)?;
    let distant = [ambient, LightInstance::new(small, Mat4::IDENTITY)];
    assert_eq!(
        capture(q, true, true, &distant)?,
        capture(q, false, true, &distant)?,
        "surfaces outside the light range must be unaffected"
    );
    let excessive = [LightInstance::new(point, Mat4::IDENTITY); 3];
    assert!(matches!(
        capture(q, true, true, &excessive)
            .err()
            .and_then(|e| e.downcast::<emergent::GraphicsError>().ok())
            .as_deref(),
        Some(emergent::GraphicsError::TooManyPointShadows)
    ));
    let directional =
        LightSource::directional(Color::WHITE, 0.4)?.with_shadow_casting(ShadowCasting::Enabled)?;
    let combined = [
        both[0],
        both[1],
        both[2],
        LightInstance::new(directional, Mat4::from_rotation_x(-0.9)),
    ];
    capture(q, true, true, &combined)?;
    println!("Point shadow smoke passed: six faces, moving/two lights, cast/receive, range and slot bounds, directional coexistence.");
    Ok(())
}
