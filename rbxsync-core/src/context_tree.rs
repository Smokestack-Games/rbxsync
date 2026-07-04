//! Assemble the flat instance array (as produced by `dom_to_instances` and the
//! Studio relay) into a single nested `datamodel.rbxjson` context document, and
//! patch that document with live deltas. Store-only AI context: scripts carry no
//! Source (only a `sourcePath` pointer), no binary, hierarchy is by nesting.

use std::collections::HashMap;
use serde_json::{json, Map, Value};

use crate::{apply_tree_mapping, normalize_path, path_with_suffix};

const SCRIPT_CLASSES: [&str; 3] = ["Script", "LocalScript", "ModuleScript"];

fn script_extension(class_name: &str) -> &'static str {
    match class_name {
        "Script" => ".server.luau",
        "LocalScript" => ".client.luau",
        _ => ".luau",
    }
}

/// Compute the project-root-relative `.luau` path a script instance maps to,
/// e.g. "src/ServerScriptService/Main.server.luau".
fn script_source_path(
    inst_path: &str,
    class_name: &str,
    src_dir_name: &str,
    tree_mapping: &HashMap<String, String>,
) -> String {
    let fs_path = apply_tree_mapping(&normalize_path(inst_path), tree_mapping);
    let joined = format!("{}/{}", src_dir_name, fs_path);
    path_with_suffix(std::path::Path::new(&joined), script_extension(class_name))
        .replace('\\', "/")
}

/// Build a leaf node object (no children yet) for one instance object.
fn make_node(inst: &Value, src_dir_name: &str, tree_mapping: &HashMap<String, String>) -> Value {
    let class_name = inst.get("className").and_then(|v| v.as_str()).unwrap_or("Folder");
    let name = inst.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let is_script = SCRIPT_CLASSES.contains(&class_name);

    let mut node = Map::new();
    node.insert("className".into(), json!(class_name));
    node.insert("name".into(), json!(name));

    if let Some(props) = inst.get("properties").and_then(|v| v.as_object()) {
        let mut props = props.clone();
        if is_script {
            props.remove("Source");
        }
        if !props.is_empty() {
            node.insert("properties".into(), Value::Object(props));
        }
    }
    if let Some(attrs) = inst.get("attributes").and_then(|v| v.as_object()) {
        if !attrs.is_empty() {
            node.insert("attributes".into(), Value::Object(attrs.clone()));
        }
    }
    if let Some(tags) = inst.get("tags").and_then(|v| v.as_array()) {
        if !tags.is_empty() {
            node.insert("tags".into(), Value::Array(tags.clone()));
        }
    }
    if is_script {
        let inst_path = inst.get("path").and_then(|v| v.as_str()).unwrap_or("");
        node.insert(
            "sourcePath".into(),
            json!(script_source_path(inst_path, class_name, src_dir_name, tree_mapping)),
        );
    }
    Value::Object(node)
}

/// Assemble the flat instance array into the nested DataModel context document.
pub fn assemble_tree(
    instances: &[Value],
    src_dir_name: &str,
    tree_mapping: &HashMap<String, String>,
) -> Value {
    // Index nodes by their DataModel path; synthesize missing ancestors as Folders.
    let mut nodes: HashMap<String, Value> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    let ensure = |path: &str, node: Value, nodes: &mut HashMap<String, Value>, order: &mut Vec<String>| {
        if !nodes.contains_key(path) {
            order.push(path.to_string());
        }
        nodes.insert(path.to_string(), node);
    };

    for inst in instances {
        let Some(path) = inst.get("path").and_then(|v| v.as_str()) else { continue };
        if path.is_empty() {
            continue;
        }
        // Synthesize any missing ancestor containers as Folders.
        let segments: Vec<&str> = path.split('/').collect();
        for i in 1..segments.len() {
            let anc = segments[..i].join("/");
            if !nodes.contains_key(&anc) {
                let anc_name = segments[i - 1];
                let placeholder = json!({ "className": "Folder", "name": anc_name });
                ensure(&anc, placeholder, &mut nodes, &mut order);
            }
        }
        let node = make_node(inst, src_dir_name, tree_mapping);
        ensure(path, node, &mut nodes, &mut order);
    }

    // Attach each node to its parent's children (root-level nodes go under DataModel).
    // Process deepest-last is unnecessary; we build children maps by path.
    let mut children_of: HashMap<String, Vec<String>> = HashMap::new();
    let mut roots: Vec<String> = Vec::new();
    for path in &order {
        match path.rfind('/') {
            Some(idx) => children_of.entry(path[..idx].to_string()).or_default().push(path.clone()),
            None => roots.push(path.clone()),
        }
    }

    fn build(path: &str, nodes: &HashMap<String, Value>, children_of: &HashMap<String, Vec<String>>) -> Value {
        let mut node = nodes.get(path).cloned().unwrap_or_else(|| json!({ "className": "Folder", "name": path }));
        if let Some(kids) = children_of.get(path) {
            let built: Vec<Value> = kids.iter().map(|k| build(k, nodes, children_of)).collect();
            if !built.is_empty() {
                node.as_object_mut().unwrap().insert("children".into(), Value::Array(built));
            }
        }
        node
    }

    let root_children: Vec<Value> = roots.iter().map(|r| build(r, &nodes, &children_of)).collect();
    json!({ "className": "DataModel", "name": "DataModel", "children": root_children })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn inst(class: &str, name: &str, path: &str, props: Value) -> Value {
        json!({ "className": class, "name": name, "path": path, "properties": props })
    }

    fn find<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
        node.get("children")?.as_array()?.iter().find(|c| c["name"] == name)
    }

    #[test]
    fn test_assembles_nested_hierarchy_with_props() {
        let tm = HashMap::new();
        let instances = vec![
            inst("Workspace", "Workspace", "Workspace", json!({"Gravity": {"type":"float","value":196.2}})),
            inst("Model", "Rig", "Workspace/Rig", json!({})),
            inst("Part", "Head", "Workspace/Rig/Head", json!({})),
        ];
        let tree = assemble_tree(&instances, "src", &tm);
        assert_eq!(tree["className"], "DataModel");
        let ws = find(&tree, "Workspace").unwrap();
        assert_eq!(ws["properties"]["Gravity"], json!({"type":"float","value":196.2}));
        let rig = find(ws, "Rig").unwrap();
        assert_eq!(rig["className"], "Model");
        assert_eq!(find(rig, "Head").unwrap()["className"], "Part");
        // Leaf has no empty children/properties keys
        assert!(find(rig, "Head").unwrap().get("children").is_none());
        assert!(find(rig, "Head").unwrap().get("properties").is_none());
    }

    #[test]
    fn test_script_node_strips_source_adds_sourcepath() {
        let tm = HashMap::new();
        let instances = vec![
            inst("ServerScriptService", "ServerScriptService", "ServerScriptService", json!({})),
            inst("Script", "Main", "ServerScriptService/Main",
                 json!({"Source": {"type":"string","value":"print('hi')"}, "Enabled": {"type":"bool","value":true}})),
        ];
        let tree = assemble_tree(&instances, "src", &tm);
        let sss = find(&tree, "ServerScriptService").unwrap();
        let main = find(sss, "Main").unwrap();
        assert!(main["properties"].get("Source").is_none());
        assert_eq!(main["properties"]["Enabled"], json!({"type":"bool","value":true}));
        assert_eq!(main["sourcePath"], "src/ServerScriptService/Main.server.luau");
    }

    #[test]
    fn test_missing_parent_synthesized_as_folder() {
        let tm = HashMap::new();
        // Only the deep leaf is provided; ancestors must be synthesized.
        let instances = vec![inst("Part", "Deep", "Workspace/A/B/Deep", json!({}))];
        let tree = assemble_tree(&instances, "src", &tm);
        let a = find(find(&tree, "Workspace").unwrap(), "A").unwrap();
        assert_eq!(a["className"], "Folder");
        assert_eq!(find(find(a, "B").unwrap(), "Deep").unwrap()["className"], "Part");
    }

    #[test]
    fn test_duplicate_sibling_paths_kept_distinct() {
        let tm = HashMap::new();
        let instances = vec![
            inst("Folder", "Root", "Root", json!({})),
            inst("Part", "P", "Root/P", json!({})),
            inst("Part", "P", "Root/P_4f37bf41", json!({})),
        ];
        let tree = assemble_tree(&instances, "src", &tm);
        let root = find(&tree, "Root").unwrap();
        assert_eq!(root["children"].as_array().unwrap().len(), 2);
    }
}
