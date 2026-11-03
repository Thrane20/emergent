//! UI assets (`*.ui.toml`): a flat `[[nodes]]` widget list with parent links, validated
//! against the widget catalogue and laid out with parent-relative anchored rects.
//!
//! Layout (mirrored in the IDE preview by `src/emergent-ide/src/lib/ui.ts`; the shared
//! fixture test keeps them equal): the root fills the canvas. A child sits in its parent's
//! content box (rect inset by `padding`). `w`/`h` are lengths (`12px`, `30%`, `100%-16px`)
//! of that box, defaulting to `100%` (`h` defaults to `24px` inside a stack). Outside a
//! stack the child is placed at `anchor`, then moved by `x`/`y` (+x right, +y down). Inside
//! a `stack` panel children run top to bottom, `gap` apart; `anchor` then only sets the
//! horizontal placement and `y` is ignored. Horizontal/vertical/grid containers
//! manage positions automatically and support nested layouts, gaps and alignment.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::component::{validate_fields, ComponentField, FieldKind};
use crate::Project;

pub const MAX_NODES: usize = 512;
pub const MAX_DEPTH: usize = 16;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_CANVAS: u32 = 4096;

#[derive(Clone, Debug, Serialize)]
pub struct WidgetSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    /// Whether the widget holds child widgets.
    pub container: bool,
    pub description: &'static str,
    pub fields: Vec<ComponentField>,
}

pub const CATEGORIES: &[&str] = &["Layout", "Text", "Controls"];
pub const ANCHORS: &[&str] = &[
    "top_left",
    "top",
    "top_right",
    "left",
    "center",
    "right",
    "bottom_left",
    "bottom",
    "bottom_right",
];

/// Every widget type: the single source for loading, the designer palette and AI prompts.
pub fn widget_catalogue() -> Vec<WidgetSpec> {
    use FieldKind::*;
    let f = ComponentField::new;
    let rgb = |r: f64, g: f64, b: f64| toml::Value::Array(vec![r.into(), g.into(), b.into()]);
    let num = |name, label, min, max, default: f64| {
        f(name, label, Number).range(min, max).default(default)
    };
    let text = || f("text", "Text", Text);
    let size = || num("size", "Text size", 1.0, 8.0, 2.0).help("Pixel-font scale.");
    let ink = || {
        f("colour", "Text colour", Colour)
            .default(rgb(0.62, 0.68, 0.72))
            .help("Linear RGB.")
    };
    let fill = |r, g, b| {
        f("fill", "Fill", Colour)
            .default(rgb(r, g, b))
            .help("Linear RGB.")
    };
    let action = || {
        f("action", "Action", Text).help(
            "Label the game binds to a function, e.g. graphics.apply. Also sent as a brain signal.",
        )
    };
    let value = || {
        f("value", "Value name", Text)
            .help("Name of the value the game reads and writes, e.g. bloom.")
    };
    let widget =
        |id, label, category, container, description, fields: Vec<ComponentField>| WidgetSpec {
            id,
            label,
            category,
            container,
            description,
            fields,
        };
    let mut widgets = vec![
        widget("panel", "Panel", "Layout", true, "A filled box that holds other widgets. Turn on Stack to list its children top to bottom.", vec![
            fill(0.012, 0.016, 0.024), f("border", "Border", Colour).default(rgb(0.25, 0.3, 0.33)), num("border_width", "Border width", 0.0, 16.0, 0.0),
            num("padding", "Padding", 0.0, 256.0, 0.0), f("stack", "Stack", Bool).default(false), num("gap", "Gap", 0.0, 256.0, 0.0),
        ]),
        widget("divider", "Divider", "Layout", false, "A horizontal rule.", vec![f("colour", "Colour", Colour).default(rgb(0.25, 0.3, 0.33)), num("thickness", "Thickness", 1.0, 16.0, 2.0)]),
        widget("label", "Label", "Text", false, "Static text: titles, headings and row names.", vec![
            text(), size(), ink(), f("align", "Align", Choice).options(&["left", "center", "right"]).default("left"),
            f("fill", "Fill", Colour).help("Optional background bar (inverted headings). Linear RGB."),
        ]),
        widget("textbox", "Text box", "Text", false, "Single-line text entry.", vec![f("text", "Placeholder", Text), action(), value(), size(), ink(), fill(0.02, 0.025, 0.035)]),
        widget("button", "Button", "Controls", false, "Sends its action when pressed. Selected buttons draw inverted (tabs).", vec![text(), action(), size(), ink(), fill(0.05, 0.06, 0.07), f("selected", "Selected", Bool).default(false)]),
        widget("toggle", "Toggle", "Controls", false, "A labelled check box.", vec![text(), action(), value(), size(), ink(), f("checked", "Checked", Bool).default(false)]),
        widget("slider", "Slider", "Controls", false, "A labelled horizontal bar between min and max.", vec![
            text(), action(), value(), size(), ink(), fill(0.45, 0.5, 0.52), num("min", "Min", -1e6, 1e6, 0.0), num("max", "Max", -1e6, 1e6, 1.0), num("default", "Default", -1e6, 1e6, 0.5),
        ]),
        widget("stepper", "Stepper", "Controls", false, "A labelled ◂ choice ▸ selector.", vec![
            text(), action(), value(), size(), ink(), f("options", "Options", Text).help("Comma-separated, e.g. Low,Medium,High,Ultra."), num("index", "Selected", 0.0, 63.0, 0.0),
        ]),
    ];
    for (id, label) in [
        ("horizontal", "Horizontal layout"),
        ("vertical", "Vertical layout"),
        ("grid", "Grid layout"),
    ] {
        widgets.push(widget(
            id,
            label,
            "Layout",
            true,
            "A nestable layout container; child positions are managed automatically.",
            vec![],
        ));
    }
    for widget in &mut widgets {
        if widget.container {
            widget.fields.extend([
                f("layout", "Layout", Choice)
                    .options(&["absolute", "horizontal", "vertical", "grid"])
                    .default(if widget.id == "panel" {
                        "absolute"
                    } else {
                        widget.id
                    }),
                f("align_main", "Main alignment", Choice)
                    .options(&["start", "center", "end"])
                    .default("start"),
                f("align_cross", "Cross alignment", Choice)
                    .options(&["start", "center", "end", "stretch"])
                    .default("start"),
                num("columns", "Grid columns", 1.0, 64.0, 2.0),
            ]);
            if widget.id != "panel" {
                widget.fields.extend([
                    num("padding", "Padding", 0.0, 256.0, 0.0),
                    num("gap", "Gap", 0.0, 256.0, 0.0),
                ]);
            }
        }

        let rect = [
            f("anchor", "Anchor", Choice)
                .options(ANCHORS)
                .default("top_left"),
            f("x", "X", Text)
                .default("0px")
                .help("px, % of the parent, or both: 50%-8px."),
            f("y", "Y", Text).default("0px"),
            f("w", "Width", Text)
                .default("100%")
                .help("px, %, or auto for content size."),
            f("h", "Height", Text).help("Defaults to 100%, or 24px inside a stack."),
        ];
        widget.fields.extend([
            num("col_span", "Column span", 1.0, 64.0, 1.0),
            num("row_span", "Row span", 1.0, 64.0, 1.0),
        ]);
        widget.fields.splice(0..0, rect);
    }
    widgets
}

/// Widget catalogue export for the designer palette, inspector forms and AI prompts.
pub fn export_widget_catalogue() -> Result<String, toml::ser::Error> {
    #[derive(Serialize)]
    struct Export {
        categories: &'static [&'static str],
        anchors: &'static [&'static str],
        widgets: Vec<WidgetSpec>,
    }
    toml::to_string(&Export {
        categories: CATEGORIES,
        anchors: ANCHORS,
        widgets: widget_catalogue(),
    })
}

/// `a% + b px`. Resolves against the parent's content size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length {
    pub percent: f32,
    pub px: f32,
}

impl Length {
    pub fn parse(text: &str) -> Option<Self> {
        let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if text.is_empty() || text.len() > 32 {
            return None;
        }
        let term = |t: &str| -> Option<Self> {
            let (number, percent) = match (t.strip_suffix('%'), t.strip_suffix("px")) {
                (Some(n), _) => (n, true),
                (_, Some(n)) => (n, false),
                _ => (t, false),
            };
            let n: f32 = number.parse().ok().filter(|n: &f32| n.is_finite())?;
            Some(if percent {
                Self {
                    percent: n,
                    px: 0.0,
                }
            } else {
                Self {
                    percent: 0.0,
                    px: n,
                }
            })
        };
        // Split at a + or - that follows a unit, so "-8px" and "1e-3" stay one term.
        let split = text.char_indices().skip(1).find(|&(i, c)| {
            matches!(c, '+' | '-') && matches!(text.as_bytes()[i - 1], b'%' | b'x')
        });
        match split {
            None => term(&text),
            Some((i, sign)) => {
                let (a, b) = (term(&text[..i])?, term(&text[i + 1..])?);
                let s = if sign == '-' { -1.0 } else { 1.0 };
                Some(Self {
                    percent: a.percent + s * b.percent,
                    px: a.px + s * b.px,
                })
            }
        }
    }
    pub fn resolve(self, size: f32) -> f32 {
        size * self.percent / 100.0 + self.px
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiFile {
    schema_version: u32,
    id: String,
    name: Option<String>,
    kind: Option<String>,
    description: Option<String>,
    canvas: [u32; 2],
    #[serde(default)]
    nodes: Vec<NodeFile>,
}

#[derive(Deserialize)]
struct NodeFile {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    parent: Option<String>,
    #[serde(flatten)]
    fields: toml::Table,
}

#[derive(Clone, Debug)]
struct Placement {
    anchor: (f32, f32),
    x: Length,
    y: Length,
    w: Length,
    h: Option<Length>,
}

#[derive(Clone, Debug)]
pub struct UiNode {
    pub id: String,
    pub kind: &'static str,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub fields: toml::Table,
    placement: Placement,
    padding: f32,
    stack: Option<f32>,
}

/// A loaded, validated UI asset. Node 0 is the root; nodes are in parent-before-child order.
#[derive(Clone, Debug)]
pub struct UiDefinition {
    pub path: String,
    pub id: String,
    pub name: String,
    pub canvas: [u32; 2],
    pub nodes: Vec<UiNode>,
}

fn identifier(id: &str, dots: bool) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || (dots && c == b'.'))
}

fn number(table: &toml::Table, name: &str, fallback: f32) -> f32 {
    match table.get(name) {
        Some(toml::Value::Float(n)) => *n as f32,
        Some(toml::Value::Integer(n)) => *n as f32,
        _ => fallback,
    }
}

impl UiDefinition {
    pub fn load(project: &Project, path: &str) -> Result<Self, String> {
        let source = project
            .read_text_asset(path)
            .map_err(|e| format!("{path}: {e}"))?;
        Self::from_source(path, &source)
    }

    /// Validates a candidate (designer or AI) against the widget catalogue.
    pub fn from_source(path: &str, source: &str) -> Result<Self, String> {
        Self::parse(path, source).map_err(|e| format!("{path}: {e}"))
    }

    fn parse(path: &str, source: &str) -> Result<Self, String> {
        if source.len() > MAX_FILE_BYTES {
            return Err("UI file exceeds 256 KiB".into());
        }
        let file: UiFile = toml::from_str(source).map_err(|e| e.to_string())?;
        let _ = file.description;
        if file.schema_version != 1 || !identifier(&file.id, true) {
            return Err("requires schema_version = 1 and a valid id".into());
        }
        if file.kind.as_deref().is_some_and(|k| k != "ui") {
            return Err("kind must be ui".into());
        }
        if file.canvas.iter().any(|&n| n == 0 || n > MAX_CANVAS) {
            return Err(format!("canvas must be 1..={MAX_CANVAS} px on each side"));
        }
        if file.nodes.len() > MAX_NODES {
            return Err(format!(
                "has {} nodes; the limit is {MAX_NODES}",
                file.nodes.len()
            ));
        }
        let catalogue = widget_catalogue();
        let mut index = HashMap::with_capacity(file.nodes.len());
        for (i, node) in file.nodes.iter().enumerate() {
            if !identifier(&node.id, false) || index.insert(node.id.as_str(), i).is_some() {
                return Err(format!("invalid or duplicate node id '{}'", node.id));
            }
        }
        let roots: Vec<_> = (0..file.nodes.len())
            .filter(|&i| file.nodes[i].parent.is_none())
            .collect();
        let [root] = roots[..] else {
            return Err(format!(
                "needs exactly one root node (no parent); found {}",
                roots.len()
            ));
        };
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); file.nodes.len()];
        let mut specs = Vec::with_capacity(file.nodes.len());
        for (i, node) in file.nodes.iter().enumerate() {
            let spec = catalogue
                .iter()
                .find(|spec| spec.id == node.kind)
                .ok_or_else(|| format!("node '{}' has unknown type '{}'", node.id, node.kind))?;
            validate_fields(
                &format!("node '{}' ({})", node.id, node.kind),
                &spec.fields,
                &node.fields,
                false,
            )?;
            for name in ["action", "value"] {
                if let Some(v) = node.fields.get(name).and_then(toml::Value::as_str) {
                    if !identifier(v, true) {
                        return Err(format!("node '{}': {name} '{v}' may only use letters, digits, '.', '_' and '-'", node.id));
                    }
                }
            }
            if let Some(parent) = &node.parent {
                let &p = index
                    .get(parent.as_str())
                    .ok_or_else(|| format!("node '{}' has missing parent '{parent}'", node.id))?;
                children[p].push(i);
            }
            specs.push(spec);
        }
        // Breadth-first from the root: parent-before-child order, depth bound, and unreachable
        // nodes (which can only be cycles, since there is one root) are rejected.
        let mut order = vec![(root, 0usize)];
        let mut cursor = 0;
        while let Some(&(i, depth)) = order.get(cursor) {
            cursor += 1;
            if !children[i].is_empty() && !specs[i].container {
                return Err(format!(
                    "node '{}' ({}) cannot hold widgets",
                    file.nodes[i].id, specs[i].id
                ));
            }
            if depth >= MAX_DEPTH && !children[i].is_empty() {
                return Err(format!("nesting is deeper than {MAX_DEPTH} levels"));
            }
            order.extend(children[i].iter().map(|&c| (c, depth + 1)));
        }
        if order.len() != file.nodes.len() {
            return Err("parent links form a cycle".into());
        }
        let mut new_index = vec![0; file.nodes.len()];
        for (new, &(old, _)) in order.iter().enumerate() {
            new_index[old] = new;
        }
        let mut nodes = Vec::with_capacity(order.len());
        for &(old, _) in &order {
            let node = &file.nodes[old];
            let length = |name: &str, fallback: &str| {
                let text = node
                    .fields
                    .get(name)
                    .and_then(toml::Value::as_str)
                    .unwrap_or(fallback);
                if matches!(name, "w" | "h") && text == "auto" {
                    return Ok(Length {
                        percent: 0.0,
                        px: 0.0,
                    });
                }
                Length::parse(text).ok_or_else(|| {
                    format!(
                        "node '{}': {name} '{text}' is not a length like 8px, 30% or 100%-16px",
                        node.id
                    )
                })
            };
            let anchor = node
                .fields
                .get("anchor")
                .and_then(toml::Value::as_str)
                .unwrap_or("top_left");
            let anchor = ANCHORS.iter().position(|a| *a == anchor).unwrap_or(0);
            let placement = Placement {
                anchor: ((anchor % 3) as f32 / 2.0, (anchor / 3) as f32 / 2.0),
                x: length("x", "0px")?,
                y: length("y", "0px")?,
                w: length("w", "100%")?,
                h: node
                    .fields
                    .contains_key("h")
                    .then(|| length("h", ""))
                    .transpose()?,
            };
            let columns = number(&node.fields, "columns", 2.0);
            if columns.fract() != 0.0
                || ["col_span", "row_span"]
                    .iter()
                    .any(|key| number(&node.fields, key, 1.0).fract() != 0.0)
            {
                return Err(format!(
                    "node '{}': columns and spans must be integers",
                    node.id
                ));
            }
            let stack = node
                .fields
                .get("stack")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            nodes.push(UiNode {
                id: node.id.clone(),
                kind: specs[old].id,
                parent: node.parent.as_ref().map(|p| new_index[index[p.as_str()]]),
                children: children[old].iter().map(|&c| new_index[c]).collect(),
                padding: number(&node.fields, "padding", 0.0),
                stack: stack.then(|| number(&node.fields, "gap", 0.0)),
                fields: node.fields.clone(),
                placement,
            });
        }
        let definition = Self {
            path: path.into(),
            name: file.name.unwrap_or_else(|| file.id.clone()),
            id: file.id,
            canvas: file.canvas,
            nodes,
        };
        for parent in &definition.nodes {
            if layout_mode(parent) == "grid" {
                grid_slots(parent, &definition.nodes)?;
            }
        }
        if definition
            .layout()
            .iter()
            .any(|r| ![r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()))
        {
            return Err("UI layout overflows finite dimensions".into());
        }
        Ok(definition)
    }

    /// Rect of every node in canvas pixels, indexed like `nodes`. Linear: parents come first.
    pub fn layout(&self) -> Vec<Rect> {
        let mut natural = vec![(0.0f32, 0.0f32); self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate().rev() {
            let text = node
                .fields
                .get("text")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            let scale = number(&node.fields, "size", 2.0);
            let mut w = (text.chars().count() as f32 * 6.0 * scale + 12.0).max(24.0);
            let mut h = 8.0 * scale + 8.0;
            if matches!(node.kind, "panel" | "horizontal" | "vertical" | "grid") {
                w = 2.0 * node.padding;
                h = 2.0 * node.padding;
            }
            if !node.children.is_empty() {
                let mode = layout_mode(node);
                let gap = number(&node.fields, "gap", 0.0);
                let mut sizes = Vec::with_capacity(node.children.len());
                for &c in &node.children {
                    let child = &self.nodes[c];
                    let (nw, nh) = natural[c];
                    sizes.push((
                        if child.fields.get("w").and_then(toml::Value::as_str) == Some("auto")
                            || !child.fields.contains_key("w")
                        {
                            nw
                        } else {
                            child.placement.w.px.max(0.0)
                        },
                        if child.fields.get("h").and_then(toml::Value::as_str) == Some("auto")
                            || child.placement.h.is_none()
                        {
                            nh
                        } else {
                            child.placement.h.map_or(nh, |h| h.px.max(0.0))
                        },
                    ));
                }
                if mode == "absolute" && node.stack.is_none() {
                    for (size, &c) in sizes.iter_mut().zip(&node.children) {
                        size.0 += self.nodes[c].placement.x.px.max(0.0);
                        size.1 += self.nodes[c].placement.y.px.max(0.0);
                    }
                }
                w = sizes.iter().map(|s| s.0).fold(0.0, f32::max);
                h = sizes.iter().map(|s| s.1).fold(0.0, f32::max);
                if mode == "horizontal" {
                    w = sizes.iter().map(|s| s.0).sum::<f32>() + gap * (sizes.len() - 1) as f32;
                } else if mode == "vertical" || node.stack.is_some() {
                    h = sizes.iter().map(|s| s.1).sum::<f32>() + gap * (sizes.len() - 1) as f32;
                } else if mode == "grid" {
                    let slots = grid_slots(node, &self.nodes).unwrap_or_default();
                    let cols = number(&node.fields, "columns", 2.0) as usize;
                    let rows = slots.iter().map(|s| s.1 + s.3).max().unwrap_or(1);
                    let cw = slots
                        .iter()
                        .zip(&sizes)
                        .map(|(s, v)| (v.0 - gap * (s.2 - 1) as f32).max(0.0) / s.2 as f32)
                        .fold(0.0, f32::max);
                    let ch = slots
                        .iter()
                        .zip(&sizes)
                        .map(|(s, v)| (v.1 - gap * (s.3 - 1) as f32).max(0.0) / s.3 as f32)
                        .fold(0.0, f32::max);
                    w = cw * cols as f32 + gap * (cols - 1) as f32;
                    h = ch * rows as f32 + gap * (rows - 1) as f32;
                }
                w += 2.0 * node.padding;
                h += 2.0 * node.padding;
            }
            natural[i] = (w, h);
        }
        let mut rects = vec![Rect::default(); self.nodes.len()];
        rects[0] = Rect {
            x: 0.0,
            y: 0.0,
            w: self.canvas[0] as f32,
            h: self.canvas[1] as f32,
        };
        for (p, parent) in self.nodes.iter().enumerate() {
            let content = Rect {
                x: rects[p].x + parent.padding,
                y: rects[p].y + parent.padding,
                w: (rects[p].w - 2.0 * parent.padding).max(0.0),
                h: (rects[p].h - 2.0 * parent.padding).max(0.0),
            };
            let mode = layout_mode(parent);
            let gap = number(&parent.fields, "gap", 0.0);
            let factor = |key| match parent
                .fields
                .get(key)
                .and_then(toml::Value::as_str)
                .unwrap_or("start")
            {
                "center" => 0.5,
                "end" => 1.0,
                _ => 0.0,
            };
            let main = factor("align_main");
            let cross = factor("align_cross");
            let stretch = parent
                .fields
                .get("align_cross")
                .and_then(toml::Value::as_str)
                == Some("stretch");
            let n = parent.children.len();
            if n == 0 {
                continue;
            }
            let columns = number(&parent.fields, "columns", 2.0) as usize;
            let slots = if mode == "grid" {
                grid_slots(parent, &self.nodes).unwrap_or_default()
            } else {
                Vec::new()
            };
            let rows = slots.iter().map(|s| s.1 + s.3).max().unwrap_or(1);
            let cell_w = if mode == "grid" {
                ((content.w - gap * (columns - 1) as f32) / columns as f32).max(0.0)
            } else {
                content.w
            };
            let cell_h = if mode == "grid" {
                ((content.h - gap * (rows - 1) as f32) / rows as f32).max(0.0)
            } else {
                content.h
            };
            let size = |index: usize| {
                let node = &self.nodes[index];
                let span_w = number(&node.fields, "col_span", 1.0);
                let span_h = number(&node.fields, "row_span", 1.0);
                let (cell_w, cell_h) = if mode == "grid" {
                    (
                        cell_w * span_w + gap * (span_w - 1.0),
                        cell_h * span_h + gap * (span_h - 1.0),
                    )
                } else {
                    (cell_w, cell_h)
                };
                let intrinsic = natural[index];
                let w = if node.fields.get("w").and_then(toml::Value::as_str) == Some("auto") {
                    intrinsic.0
                } else if mode == "horizontal" && !node.fields.contains_key("w") {
                    24.0
                } else {
                    node.placement.w.resolve(cell_w).max(0.0)
                };
                let h = if node.fields.get("h").and_then(toml::Value::as_str) == Some("auto") {
                    intrinsic.1
                } else {
                    node.placement
                        .h
                        .map(|h| h.resolve(cell_h))
                        .unwrap_or(
                            if mode == "vertical" || mode == "absolute" && parent.stack.is_some() {
                                24.0
                            } else {
                                cell_h
                            },
                        )
                        .max(0.0)
                };
                (
                    if stretch && mode == "vertical" {
                        cell_w
                    } else {
                        w
                    },
                    if stretch && mode == "horizontal" {
                        cell_h
                    } else {
                        h
                    },
                )
            };
            let used: f32 = parent
                .children
                .iter()
                .map(|&c| {
                    let (w, h) = size(c);
                    if mode == "horizontal" {
                        w
                    } else {
                        h
                    }
                })
                .sum::<f32>()
                + gap * (n - 1) as f32;
            let mut cursor = if mode == "horizontal" {
                (content.w - used).max(0.0) * main
            } else if mode == "vertical" {
                (content.h - used).max(0.0) * main
            } else {
                0.0
            };
            for (slot, &c) in parent.children.iter().enumerate() {
                let node = &self.nodes[c];
                let at = &node.placement;
                let (w, mut h) = size(c);
                let (x, y) = match mode {
                    "horizontal" => {
                        let x = content.x + cursor;
                        cursor += w + gap;
                        (x, content.y + (content.h - h).max(0.0) * cross)
                    }
                    "vertical" => {
                        let y = content.y + cursor;
                        cursor += h + gap;
                        (content.x + (content.w - w).max(0.0) * cross, y)
                    }
                    "grid" => {
                        let (col, row, cs, rs) = slots.get(slot).copied().unwrap_or((0, 0, 1, 1));
                        let cw = cell_w * cs as f32 + gap * (cs - 1) as f32;
                        let ch = cell_h * rs as f32 + gap * (rs - 1) as f32;
                        if stretch {
                            h = ch;
                        }
                        (
                            content.x + col as f32 * (cell_w + gap) + (cw - w).max(0.0) * main,
                            content.y + row as f32 * (cell_h + gap) + (ch - h).max(0.0) * cross,
                        )
                    }
                    _ => {
                        let x = content.x + at.anchor.0 * (content.w - w) + at.x.resolve(content.w);
                        let y = if let Some(gap) = parent.stack {
                            let y = content.y + cursor;
                            cursor += h + gap;
                            y
                        } else {
                            content.y + at.anchor.1 * (content.h - h) + at.y.resolve(content.h)
                        };
                        (x, y)
                    }
                };
                rects[c] = Rect { x, y, w, h };
            }
        }
        rects
    }

    /// Every action label in the asset, in node order, without repeats.
    pub fn actions(&self) -> Vec<&str> {
        let mut actions: Vec<&str> = Vec::new();
        for action in self
            .nodes
            .iter()
            .filter_map(|n| n.fields.get("action").and_then(toml::Value::as_str))
        {
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
        actions
    }
}

fn layout_mode(node: &UiNode) -> &str {
    node.fields
        .get("layout")
        .and_then(toml::Value::as_str)
        .unwrap_or(if matches!(node.kind, "horizontal" | "vertical" | "grid") {
            node.kind
        } else {
            "absolute"
        })
}
fn grid_slots(
    parent: &UiNode,
    nodes: &[UiNode],
) -> Result<Vec<(usize, usize, usize, usize)>, String> {
    let columns = number(&parent.fields, "columns", 2.0) as usize;
    let mut occupied = vec![false; columns * 512];
    let mut slots = Vec::with_capacity(parent.children.len());
    for &c in &parent.children {
        let cs = number(&nodes[c].fields, "col_span", 1.0) as usize;
        let rs = number(&nodes[c].fields, "row_span", 1.0) as usize;
        if cs > columns {
            return Err(format!(
                "node {}: column span exceeds grid columns",
                nodes[c].id
            ));
        }
        let found = (0..columns * 512).find(|&at| {
            let col = at % columns;
            let row = at / columns;
            col + cs <= columns
                && row + rs <= 512
                && (row..row + rs).all(|y| (col..col + cs).all(|x| !occupied[y * columns + x]))
        });
        let at = found.ok_or("grid spans exceed 512 rows")?;
        let col = at % columns;
        let row = at / columns;
        for y in row..row + rs {
            for x in col..col + cs {
                occupied[y * columns + x] = true;
            }
        }
        slots.push((col, row, cs, rs));
    }
    Ok(slots)
}
