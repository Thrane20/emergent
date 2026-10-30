//! Native image regressions for independent object shadow controls.
use emergent::{
    Color, Geometry, GeometryInstance, GeometryRenderer, LightInstance, LightSource, Mat4,
    MaterialDefinition, Primitive, ShadowCasting, Vec3,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(384, 256))?;
    renderer.set_view(Vec3::new(7.0, 6.0, 9.0), Vec3::ZERO, Vec3::Y)?;
    let cube = Geometry::from(Primitive::cube(0.55, Color::CYAN)?);
    let floor = Geometry::from(Primitive::plane(1.5, 3.5, Color::WHITE)?);
    let sunlight =
        LightSource::directional(Color::WHITE, 0.9)?.with_shadow_casting(ShadowCasting::Enabled)?;
    let lights = [
        LightInstance::new(LightSource::ambient(Color::WHITE, 0.15)?, Mat4::IDENTITY),
        LightInstance::new(sunlight, Mat4::from_rotation_x(-0.9)),
    ];
    let mut capture = |cast: bool,
                       receive: bool,
                       right_cast: bool|
     -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut instances = Vec::new();
        for (x, cast, receive) in [(-2.0, cast, receive), (2.0, right_cast, true)] {
            let caster = MaterialDefinition {
                cast_shadows: cast,
                receive_shadows: false,
                ..Default::default()
            }
            .constant()?;
            let receiver = MaterialDefinition {
                cast_shadows: false,
                receive_shadows: receive,
                ..Default::default()
            }
            .constant()?;
            let c = renderer.insert_with_material(&cube, &caster)?;
            let f = renderer.insert_with_material(&floor, &receiver)?;
            instances.push(GeometryInstance::new(
                c,
                Mat4::from_translation(Vec3::new(x, 1.6, 0.0)),
            ));
            instances.push(GeometryInstance::new(
                f,
                Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
            ));
        }
        let image = renderer.render_lit_rgba(&instances, &lights)?.rgba;
        for instance in instances {
            assert!(renderer.remove(instance.geometry));
        }
        Ok(image)
    };
    let enabled = capture(true, true, true)?;
    let cast_off = capture(false, true, true)?;
    let receive_off = capture(true, false, true)?;
    let both_off = capture(false, false, true)?;
    let all_off = capture(false, false, false)?;
    let changed = |a: &[u8], b: &[u8]| a.iter().zip(b).filter(|(a, b)| a != b).count();
    assert!(
        changed(&enabled, &cast_off) > 100,
        "caster toggle must remove its visible floor shadow"
    );
    assert_eq!(
        receive_off, cast_off,
        "disabling the receiver removes only that floor's shadow"
    );
    assert_eq!(
        both_off, cast_off,
        "cast and receive options must be independent"
    );
    assert!(
        changed(&both_off, &all_off) > 100,
        "other objects must still receive their shadows"
    );
    println!("Object shadow smoke passed on {} / {}: left toggle {} changed bytes; other receiver {} changed bytes",renderer.info().adapter_name,renderer.info().backend,changed(&enabled,&cast_off),changed(&both_off,&all_off));
    Ok(())
}
