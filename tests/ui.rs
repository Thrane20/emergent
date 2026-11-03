use emergent::ui::{export_widget_catalogue, Length, UiDefinition};

const OPTIONS: &str = include_str!("../templates/standard/assets/ui/options.ui.toml");
/// Expected rects shared with the IDE preview test (`src/emergent-ide/tests/ui.test.ts`).
const LAYOUT: &str = include_str!("fixtures/ui-options-layout.toml");

fn ui(nodes: &str) -> Result<UiDefinition, String> {
    UiDefinition::from_source(
        "assets/ui/t.ui.toml",
        &format!("schema_version=1\nid='t'\nkind='ui'\ncanvas=[100,50]\n{nodes}"),
    )
}

#[test]
fn lengths_parse_percent_and_pixels() {
    assert_eq!(
        Length::parse("100%-16px"),
        Some(Length {
            percent: 100.0,
            px: -16.0
        })
    );
    assert_eq!(
        Length::parse("-8px"),
        Some(Length {
            percent: 0.0,
            px: -8.0
        })
    );
    assert_eq!(
        Length::parse("12"),
        Some(Length {
            percent: 0.0,
            px: 12.0
        })
    );
    assert_eq!(Length::parse("50% + 4px").unwrap().resolve(200.0), 104.0);
    for bad in ["", "px", "10em", "5%%", "1-2"] {
        assert_eq!(Length::parse(bad), None, "{bad}");
    }
}

#[test]
fn sample_layout_matches_shared_fixture() {
    let ui = UiDefinition::from_source("assets/ui/options.ui.toml", OPTIONS).unwrap();
    let expected: toml::Table = toml::from_str(LAYOUT).unwrap();
    let rects = ui.layout();
    assert_eq!(expected.len(), ui.nodes.len());
    for (node, rect) in ui.nodes.iter().zip(&rects) {
        let want: Vec<f32> = expected[&node.id]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                v.as_integer()
                    .map(|n| n as f32)
                    .or(v.as_float().map(|n| n as f32))
                    .unwrap()
            })
            .collect();
        let got = [rect.x, rect.y, rect.w, rect.h];
        assert!(
            want.iter().zip(got).all(|(a, b)| (a - b).abs() < 1e-3),
            "{}: {got:?} != {want:?}",
            node.id
        );
    }
    assert_eq!(ui.actions()[0], "options.tab.graphics");
}

#[test]
fn anchors_and_stacks() {
    let ui = ui("[[nodes]]\nid='r'\ntype='panel'\npadding=5\n[[nodes]]\nid='a'\ntype='label'\nparent='r'\nanchor='bottom_right'\nw='10px'\nh='10px'\nx='-1px'\n[[nodes]]\nid='s'\ntype='panel'\nparent='r'\nstack=true\ngap=2\nw='50%'\n[[nodes]]\nid='b1'\ntype='button'\nparent='s'\n[[nodes]]\nid='b2'\ntype='button'\nparent='s'\nh='4px'\nanchor='top_right'\nw='10px'").unwrap();
    let r = ui.layout();
    let at = |id: &str| {
        let i = ui.nodes.iter().position(|n| n.id == id).unwrap();
        [r[i].x, r[i].y, r[i].w, r[i].h]
    };
    assert_eq!(at("a"), [84.0, 35.0, 10.0, 10.0]);
    assert_eq!(at("b1"), [5.0, 5.0, 45.0, 24.0]);
    assert_eq!(at("b2"), [40.0, 31.0, 10.0, 4.0]);
}

#[test]
fn rejects_broken_trees_and_fields() {
    let cases = [
        ("", "exactly one root"),
        ("[[nodes]]\nid='r'\ntype='panel'\n[[nodes]]\nid='r'\ntype='label'\nparent='r'", "duplicate"),
        ("[[nodes]]\nid='r'\ntype='label'\n[[nodes]]\nid='c'\ntype='label'\nparent='r'", "cannot hold"),
        ("[[nodes]]\nid='r'\ntype='panel'\n[[nodes]]\nid='a'\ntype='panel'\nparent='b'\n[[nodes]]\nid='b'\ntype='panel'\nparent='a'", "cycle"),
        ("[[nodes]]\nid='r'\ntype='window'", "unknown type"),
        ("[[nodes]]\nid='r'\ntype='panel'\n[[nodes]]\nid='q'\ntype='panel'\nparent='x'", "missing parent"),
        ("[[nodes]]\nid='r'\ntype='button'\naction='fire now'", "may only use"),
        ("[[nodes]]\nid='r'\ntype='button'\nw='wide'", "not a length"),
        ("[[nodes]]\nid='r'\ntype='button'\nbogus=1", "no field"),
    ];
    for (nodes, message) in cases {
        let error = ui(nodes).unwrap_err();
        assert!(error.contains(message), "{nodes}: {error}");
    }
    let deep: String = (0..18)
        .map(|i| {
            let parent = if i > 0 {
                format!("parent='n{}'", i - 1)
            } else {
                String::new()
            };
            format!("[[nodes]]\nid='n{i}'\ntype='panel'\n{parent}\n")
        })
        .collect();
    assert!(ui(&deep).unwrap_err().contains("deeper"));
}

#[test]
fn catalogue_exports() {
    let text = export_widget_catalogue().unwrap();
    for id in [
        "panel", "label", "button", "toggle", "slider", "stepper", "textbox", "divider",
    ] {
        assert!(text.contains(&format!("id = \"{id}\"")), "{id}");
    }
}

#[test]
fn validates_asset_limits_and_orders_children_after_parents() {
    let loaded = ui(
        "[[nodes]]\nid='child'\ntype='label'\nparent='root'\n[[nodes]]\nid='root'\ntype='panel'",
    )
    .unwrap();
    assert_eq!(loaded.nodes[0].id, "root");
    assert_eq!(loaded.nodes[1].parent, Some(0));
    assert_eq!(loaded.layout()[1].w, 100.0);
    let nodes: String = (0..513)
        .map(|i| {
            let parent = if i == 0 {
                String::new()
            } else {
                "parent='n0'\n".into()
            };
            format!("[[nodes]]\nid='n{i}'\ntype='panel'\n{parent}")
        })
        .collect();
    assert!(ui(&nodes).unwrap_err().contains("limit is 512"));
    for canvas in ["[0,50]", "[4097,50]", "[1.5,50]"] {
        assert!(UiDefinition::from_source(
            "bad.ui.toml",
            &format!(
                "schema_version=1\nid='t'\ncanvas={canvas}\n[[nodes]]\nid='root'\ntype='panel'"
            )
        )
        .is_err());
    }
}

#[test]
fn nested_layouts_match_shared_rects_and_reject_fractional_columns() {
    let source = include_str!("fixtures/ui-nested-layout.ui.toml");
    let ui = UiDefinition::from_source("nested", source).unwrap();
    let expected: toml::Table =
        toml::from_str(include_str!("fixtures/ui-nested-layout-rects.toml")).unwrap();
    for (node, rect) in ui.nodes.iter().zip(ui.layout()) {
        let want = expected[&node.id].as_array().unwrap();
        for (value, actual) in want.iter().zip([rect.x, rect.y, rect.w, rect.h]) {
            let value = value
                .as_float()
                .unwrap_or_else(|| value.as_integer().unwrap() as f64)
                as f32;
            assert!((value - actual).abs() < 0.001, "{}: {rect:?}", node.id);
        }
    }
    assert!(
        UiDefinition::from_source("nested", &source.replace("columns=2", "columns=2.5")).is_err()
    );
}

#[test]
fn flow_stretch_defaults_and_panel_mode_are_supported() {
    for (mode, w, h) in [("horizontal", 24., 50.), ("vertical", 100., 24.)] {
        let source=format!("[[nodes]]\nid='r'\ntype='panel'\nlayout='{mode}'\nalign_cross='stretch'\n[[nodes]]\nid='a'\nparent='r'\ntype='button'\n");
        let ui = ui(&source).unwrap();
        let rect = ui.layout()[1];
        assert_eq!((rect.w, rect.h), (w, h));
    }
}

#[test]
fn spans_pack_without_overlap_and_auto_measures_nested_content() {
    let source="schema_version=1\nid='span'\ncanvas=[120,120]\n[[nodes]]\nid='r'\ntype='grid'\ncolumns=3\n[[nodes]]\nid='a'\nparent='r'\ntype='label'\ncol_span=2\nrow_span=2\n[[nodes]]\nid='b'\nparent='r'\ntype='label'\n[[nodes]]\nid='c'\nparent='r'\ntype='label'\n";
    let ui = UiDefinition::from_source("span", source).unwrap();
    let r = ui.layout();
    assert_eq!((r[1].x, r[1].y, r[1].w, r[1].h), (0., 0., 80., 120.));
    assert_eq!((r[2].x, r[2].y), (80., 0.));
    assert_eq!((r[3].x, r[3].y), (80., 60.));
    assert!(
        UiDefinition::from_source("span", &source.replace("col_span=2", "col_span=4")).is_err()
    );
    let ui=crate::ui("[[nodes]]\nid='r'\ntype='panel'\n[[nodes]]\nid='row'\nparent='r'\ntype='horizontal'\nw='auto'\nh='auto'\npadding=2\ngap=3\n[[nodes]]\nid='a'\nparent='row'\ntype='button'\nw='10px'\nh='8px'\n[[nodes]]\nid='b'\nparent='row'\ntype='button'\nw='20px'\nh='12px'").unwrap();
    let r = ui.layout();
    assert_eq!((r[1].w, r[1].h), (37., 16.));
}
