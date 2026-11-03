//! Bounded surface shader graphs. Authored IDs are resolved while compiling, never on a draw path.
use crate::MaterialError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderParameter {
    pub id: String,
    pub name: String,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderOp {
    Constant,
    Parameter,
    Time,
    PositionX,
    PositionY,
    PositionZ,
    UvX,
    UvY,
    Fresnel,
    Noise,
    Add,
    Multiply,
    Sine,
    OneMinus,
    Smoothstep,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderNode {
    pub id: String,
    pub op: ShaderOp,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub value: f32,
    pub parameter: Option<String>,
    /// Editor layout only: where the node card sits on the graph canvas. Ignored by compilation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 2]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderOutputs {
    pub color: [String; 3],
    pub emission: String,
    pub opacity: String,
    pub displacement: String,
    /// Editor layout only: where the surface output card sits on the graph canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 2]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub transparent: bool,
    pub parameters: Vec<ShaderParameter>,
    pub nodes: Vec<ShaderNode>,
    pub outputs: ShaderOutputs,
}
fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn layout(position: Option<[f32; 2]>) -> bool {
    position.is_none_or(|p| p.iter().all(|v| v.is_finite() && v.abs() <= 100_000.))
}
impl ShaderDefinition {
    pub fn validate(&self) -> Result<(), MaterialError> {
        let fail = |text: &str| MaterialError(format!("Shader: {text}"));
        if self.schema_version != 1
            || !identifier(&self.id)
            || self.name.trim().is_empty()
            || self.name.chars().count() > 64
            || self.name.chars().any(char::is_control)
            || self.nodes.is_empty()
            || self.nodes.len() > 32
            || self.parameters.len() > 16
        {
            return Err(fail(
                "use schema 1, an ID/name, 1–32 nodes and at most 16 parameters",
            ));
        }
        let mut params = HashMap::new();
        for p in &self.parameters {
            if !identifier(&p.id)
                || p.name.trim().is_empty()
                || p.name.chars().count() > 64
                || p.name.chars().any(char::is_control)
                || ![p.min, p.max, p.default]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 10000.)
                || p.min > p.max
                || !(p.min..=p.max).contains(&p.default)
                || params.insert(&p.id, p).is_some()
            {
                return Err(fail("invalid or duplicate parameter"));
            }
        }
        let mut nodes = HashMap::new();
        for n in &self.nodes {
            use ShaderOp::*;
            let arity = match n.op {
                Add | Multiply => 2,
                Sine | OneMinus | Noise => 1,
                Smoothstep => 3,
                _ => 0,
            };
            if !identifier(&n.id)
                || nodes.contains_key(&n.id)
                || n.inputs.len() != arity
                || !n.value.is_finite()
                || n.value.abs() > 10000.
                || n.inputs.iter().any(|id| !nodes.contains_key(id))
                || (n.op == Parameter
                    && !n.parameter.as_ref().is_some_and(|p| params.contains_key(p)))
                || (n.op != Parameter && n.parameter.is_some())
                || !layout(n.position)
            {
                return Err(fail(&format!(
                    "node {}: check operation, unique ID, parameter and earlier input references",
                    n.id
                )));
            }
            let view_dependent = n.op == Fresnel || n.inputs.iter().any(|id| nodes[id]);
            nodes.insert(&n.id, view_dependent);
        }
        let out = &self.outputs;
        if out
            .color
            .iter()
            .chain([&out.emission, &out.opacity, &out.displacement])
            .any(|id| !nodes.contains_key(id))
        {
            return Err(fail("output references a missing node"));
        }
        if !layout(out.position) {
            return Err(fail(
                "output position must be two finite numbers within ±100000",
            ));
        }
        if nodes[&out.displacement] {
            return Err(fail(
                "vertex displacement cannot depend on view-facing/Fresnel",
            ));
        }
        Ok(())
    }
    pub fn parameter_values(
        &self,
        overrides: &BTreeMap<String, f32>,
    ) -> Result<[[f32; 4]; 4], MaterialError> {
        self.validate()?;
        if overrides
            .keys()
            .any(|id| !self.parameters.iter().any(|p| &p.id == id))
        {
            return Err(MaterialError("Unknown shader parameter override".into()));
        }
        let mut result = [[0.; 4]; 4];
        for (i, p) in self.parameters.iter().enumerate() {
            let value = overrides.get(&p.id).copied().unwrap_or(p.default);
            if !value.is_finite() || !(p.min..=p.max).contains(&value) {
                return Err(MaterialError(format!(
                    "Shader parameter {} is outside {}..{}",
                    p.id, p.min, p.max
                )));
            }
            result[i / 4][i % 4] = value;
        }
        Ok(result)
    }
    /// Generates only expressions from validated operations, never interpolated author code.
    pub fn wgsl(&self) -> Result<String, MaterialError> {
        self.validate()?;
        let mut code =
            String::from("fn surface_graph(p:vec3<f32>,uv:vec2<f32>,facing:f32)->SurfaceGraph {\n");
        let indices: HashMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (&n.id, i))
            .collect();
        for (i, n) in self.nodes.iter().enumerate() {
            use ShaderOp::*;
            let args: Vec<_> = n
                .inputs
                .iter()
                .map(|id| format!("n{}", indices[id]))
                .collect();
            let expr = match n.op {
                Constant => format!("{:.8}", n.value),
                Parameter => {
                    let i = self
                        .parameters
                        .iter()
                        .position(|p| Some(&p.id) == n.parameter.as_ref())
                        .unwrap();
                    format!("object.material.shader_parameters[{0}][{1}]", i / 4, i % 4)
                }
                Time => "object.material.shader_time.x".into(),
                PositionX => "p.x".into(),
                PositionY => "p.y".into(),
                PositionZ => "p.z".into(),
                UvX => "uv.x".into(),
                UvY => "uv.y".into(),
                Fresnel => "clamp(1.0-abs(facing),0.0,1.0)".into(),
                Noise => format!("surface_noise({})", args[0]),
                Add => format!("({}+{})", args[0], args[1]),
                Multiply => format!("({}*{})", args[0], args[1]),
                Sine => format!("sin({})", args[0]),
                OneMinus => format!("(1.0-{})", args[0]),
                Smoothstep => format!("surface_smooth({},{},{})", args[0], args[1], args[2]),
            };
            // Keep intermediate arithmetic bounded even for adversarial but structurally valid graphs.
            code.push_str(&format!("let n{i}=clamp({expr},-10000.0,10000.0);\n"));
        }
        let o = &self.outputs;
        code.push_str(&format!("return SurfaceGraph(clamp(vec3<f32>(n{},n{},n{}),vec3<f32>(0.0),vec3<f32>(1.0)),clamp(n{},0.0,100.0),clamp(n{},0.0,1.0),clamp(n{},-100.0,100.0));\n}}",indices[&o.color[0]],indices[&o.color[1]],indices[&o.color[2]],indices[&o.emission],indices[&o.opacity],indices[&o.displacement]));
        Ok(code)
    }
}
