use crate::source;
use crate::xur::{self, Object, Value};
use std::collections::HashSet;
use std::path::PathBuf;

fn builds() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common")
}

fn archives() -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut stack = vec![builds()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "xzp") {
                found.push((p.display().to_string(), std::fs::read(p).unwrap()));
            }
        }
    }
    found.extend(crate::tests_fixtures::packages().into_iter().flat_map(|(role, id, files)| {
        files
            .into_iter()
            .filter(|f| f.name.to_lowercase().ends_with(".xzp"))
            .map(move |f| (format!("{role}/{id}/{}", f.name), f.payload.read().unwrap()))
    }));
    found
}

fn scenes_in(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    source::expand_named("a.xzp", data)
        .ok()
        .flatten()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.name.ends_with(".xur"))
        .map(|e| (e.name, e.payload.read().unwrap()))
        .collect()
}

fn bug() -> Vec<u8> {
    let archive = std::fs::read(builds().join("v1.0.808.0_debug_unmodified/media/launch.xzp")).unwrap();
    scenes_in(&archive).into_iter().find(|(n, _)| n == "Bug.xur").unwrap().1
}

fn count(o: &Object) -> usize {
    1 + o.children.iter().map(count).sum::<usize>()
}

fn find<'a>(o: &'a Object, class: &str) -> Option<&'a Object> {
    if o.class == class {
        return Some(o);
    }
    o.children.iter().find_map(|c| find(c, class))
}

fn property<'a>(o: &'a Object, name: &str) -> Option<&'a Vec<Value>> {
    o.groups.iter().flat_map(|g| &g.props).find(|p| p.name == name).map(|p| &p.values)
}

#[test]
fn every_scene_in_the_game_files_parses_to_the_last_byte() {
    let mut seen = HashSet::new();
    let (mut parsed, mut failures) = (0, Vec::new());
    for (archive_name, archive) in archives() {
        for (name, data) in scenes_in(&archive) {
            if !seen.insert(data.clone()) {
                continue;
            }
            match xur::parse_scene(&data) {
                Ok(_) => parsed += 1,
                Err(e) => failures.push(format!("{archive_name}/{name}: {e}")),
            }
        }
    }
    assert!(parsed >= 100, "only {parsed} parsed");
    assert!(failures.iter().all(|f| f.contains("XuiHtmlPresenter")), "{failures:#?}");
    assert!(failures.len() <= 1);
}

#[test]
fn bug_scene_tree_matches_the_python_reference() {
    let scene = xur::parse_scene(&bug()).unwrap();
    assert_eq!(scene.root.class, "XuiCanvas");
    assert_eq!(count(&scene.root), 27);
    assert_eq!(property(&scene.root, "Width"), Some(&vec![Value::Float(1280.0)]));
    assert_eq!(property(&scene.root, "Height"), Some(&vec![Value::Float(720.0)]));
    let inner = find(&scene.root, "XuiScene").unwrap();
    assert_eq!(property(inner, "Id"), Some(&vec![Value::Text("Bug_scene".into())]));
    assert_eq!(property(inner, "TransTo"), Some(&vec![Value::Text("TransTo".into())]));
}

#[test]
fn vectors_resolve_through_the_vect_section() {
    let scene = xur::parse_scene(&bug()).unwrap();
    let group = find(&scene.root, "XuiGroup").unwrap();
    assert_eq!(property(group, "Position"), Some(&vec![Value::Vector([84.06571, 612.0, 0.0])]));
    assert_eq!(property(group, "Pivot"), Some(&vec![Value::Vector([100.0, 32.0, 0.0])]));
}

#[test]
fn compound_properties_expose_nested_values() {
    let scene = xur::parse_scene(&bug()).unwrap();
    let figure = find(&scene.root, "XuiFigure").unwrap();
    let fill = property(figure, "Fill").unwrap();
    let Value::Compound(inner) = &fill[0] else { panic!("Fill is not compound: {fill:?}") };
    assert!(inner.iter().any(|p| p.name == "FillType" && p.values == vec![Value::Uint(2)]));
    assert!(inner.iter().any(|p| p.name == "Gradient"));
}

#[test]
fn text_elements_carry_colour_size_and_strings() {
    let scene = xur::parse_scene(&bug()).unwrap();
    let text = find(&scene.root, "XuiText").unwrap();
    assert_eq!(property(text, "TextColor"), Some(&vec![Value::Color(0xFFFFFFFF)]));
    assert_eq!(property(text, "DropShadowColor"), Some(&vec![Value::Color(0x800F0F0F)]));
    assert_eq!(property(text, "PointSize"), Some(&vec![Value::Float(38.0)]));
    assert_eq!(property(text, "Text"), Some(&vec![Value::Text("2:00".into())]));
}

#[test]
fn timelines_list_named_frames_and_animated_targets() {
    let scene = xur::parse_scene(&bug()).unwrap();
    let owner = find(&scene.root, "XuiScene").unwrap();
    let timeline = owner.timeline.as_ref().unwrap();
    assert_eq!(timeline.frames.len(), 12);
    assert_eq!((timeline.frames[0].name.as_str(), timeline.frames[0].frame), ("TransTo", 0));
    assert_eq!(timeline.frames[1].name, "EndTransTo");
    let targets: Vec<_> = timeline.tracks.iter().map(|t| (t.target.as_str(), t.keys.len())).collect();
    assert_eq!(&targets[..3], [("bugFrame", 17), ("bugGroup", 17), ("glowBurst", 6)]);
    assert!(timeline.tracks[0].paths.iter().any(|p| p == "XuiElement.Opacity"), "{:?}", timeline.tracks[0].paths);
}

#[test]
fn rendered_tree_is_indented_and_readable() {
    let lines = xur::render(&xur::parse_scene(&bug()).unwrap());
    assert!(lines.iter().any(|l| l == "XuiCanvas"), "{:#?}", &lines[..6]);
    assert!(lines.iter().any(|l| l.trim() == "XuiScene \"Bug_scene\""));
    assert!(lines.iter().any(|l| l.trim() == "Width = 1280"));
    assert!(lines.iter().any(|l| l.trim() == "TextColor = #FFFFFFFF"));
    assert!(lines.iter().any(|l| l.trim().starts_with("Position = (84.06571, 612, 0)")));
    assert!(lines.iter().any(|l| l.trim() == "named frame 0 TransTo"));
}

#[test]
fn unknown_class_is_reported_by_name() {
    let archive = std::fs::read(builds().join("R1Shell/media/r1shell.xzp")).unwrap();
    let (_, data) = scenes_in(&archive).into_iter().find(|(n, _)| n.contains("skin_default")).unwrap();
    let err = xur::parse_scene(&data).unwrap_err().to_string();
    assert!(err.contains("XuiHtmlPresenter"), "{err}");
}

#[test]
fn truncated_data_is_an_error_not_a_panic() {
    let mut data = bug();
    data.truncate(data.len() - 40);
    assert!(xur::parse_scene(&data).is_err());
}
