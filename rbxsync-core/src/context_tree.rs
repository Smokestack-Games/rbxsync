//! Assemble the flat instance array (as produced by `dom_to_instances` and the
//! Studio relay) into a single nested `datamodel.rbxjson` context document, and
//! patch that document with live deltas. Store-only AI context: scripts carry no
//! Source (only a `sourcePath` pointer), no binary, hierarchy is by nesting.

use std::collections::HashMap;
use std::path::Path;
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

/// Assemble the instances into the nested document and write it atomically to
/// `<project_dir>/datamodel.rbxjson`, then stamp snapshot freshness (full extract).
pub fn write_context_file(
    project_dir: &Path,
    instances: &[Value],
    tree_mapping: &HashMap<String, String>,
) -> std::io::Result<usize> {
    // `dom_to_instances` gives duplicate-named siblings the SAME flat `path`
    // (identity lives in `referenceId`). Rewrite each instance's `path` to its
    // disambiguated path so all siblings stay distinct nodes in the nested
    // document — using the same scheme plan_instance_writes uses for the
    // `.luau`/`.rbxjson` files, so a duplicate script's `sourcePath` matches its
    // real filename. The node `name` (raw name) is left untouched; only the
    // path used for nesting/keying is disambiguated.
    let ref_to_path = crate::extract_tree::disambiguate_paths(instances);
    let disambiguated: Vec<Value> = instances
        .iter()
        .map(|inst| {
            let ref_id = inst.get("referenceId").and_then(|v| v.as_str()).unwrap_or("");
            match ref_to_path.get(ref_id) {
                Some(new_path) => {
                    let mut inst = inst.clone();
                    if let Some(obj) = inst.as_object_mut() {
                        obj.insert("path".into(), json!(new_path));
                    }
                    inst
                }
                None => inst.clone(),
            }
        })
        .collect();
    let tree = assemble_tree(&disambiguated, "src", tree_mapping);
    let json = serde_json::to_string_pretty(&tree).map_err(std::io::Error::other)?;
    let target = project_dir.join("datamodel.rbxjson");
    let tmp = project_dir.join("datamodel.rbxjson.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &target)?;
    crate::extract_tree::write_snapshot_freshness(&project_dir.to_string_lossy(), true);
    Ok(instances.len())
}

#[cfg(test)]
mod write_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_write_context_file_produces_single_nested_doc() {
        let dir = tempfile::tempdir().unwrap();
        let insts = vec![
            json!({"className":"Workspace","name":"Workspace","path":"Workspace","properties":{}}),
            json!({"className":"Part","name":"Part","path":"Workspace/Part","properties":{}}),
        ];
        write_context_file(dir.path(), &insts, &HashMap::new()).unwrap();
        let doc: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("datamodel.rbxjson")).unwrap()).unwrap();
        assert_eq!(doc["className"], "DataModel");
        assert_eq!(doc["children"][0]["name"], "Workspace");
        assert_eq!(doc["children"][0]["children"][0]["name"], "Part");
        assert!(dir.path().join(".rbxsync/snapshot.json").exists());
    }
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
    fn test_duplicate_siblings_from_real_dom_all_present() {
        use rbx_dom_weak::{InstanceBuilder, WeakDom};
        let dom = WeakDom::new(InstanceBuilder::new("DataModel").with_child(
            InstanceBuilder::new("Workspace").with_name("Workspace").with_children([
                InstanceBuilder::new("Part").with_name("Part"),
                InstanceBuilder::new("Part").with_name("Part"),
                InstanceBuilder::new("Part").with_name("Part"),
            ])));
        let instances = crate::dom_to_instances(&dom);
        let dir = tempfile::tempdir().unwrap();
        crate::context_tree::write_context_file(dir.path(), &instances, &std::collections::HashMap::new()).unwrap();
        let doc: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("datamodel.rbxjson")).unwrap()).unwrap();
        let ws = doc["children"].as_array().unwrap().iter().find(|c| c["name"]=="Workspace").unwrap();
        let parts: Vec<_> = ws["children"].as_array().unwrap().iter().filter(|c| c["name"]=="Part").collect();
        assert_eq!(parts.len(), 3, "all three same-named siblings must survive in the context doc");
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpKind { Create, Modify, Delete, Rename }

/// One live delta to apply to the context document. `path` is the DataModel
/// path; `new_path` is set for renames; `data` is the full instance object
/// (create/modify) in the same shape `assemble_tree` consumes.
#[derive(Debug, Clone)]
pub struct SyncOp {
    pub op: OpKind,
    pub path: String,
    pub new_path: Option<String>,
    pub data: Option<Value>,
}

/// Navigate to the mutable children Vec of the parent of `path`, creating
/// missing ancestor Folders. Returns (parent_children, leaf_name).
fn parent_children<'a>(root: &'a mut Value, path: &str) -> Option<(&'a mut Vec<Value>, String)> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }
    let leaf = segments[segments.len() - 1].to_string();
    let mut cur = root;
    for seg in &segments[..segments.len() - 1] {
        // ensure children array exists
        if cur.get("children").is_none() {
            cur.as_object_mut()?.insert("children".into(), Value::Array(vec![]));
        }
        let kids = cur.get_mut("children")?.as_array_mut()?;
        let idx = kids.iter().position(|c| c.get("name").and_then(|n| n.as_str()) == Some(*seg));
        let idx = match idx {
            Some(i) => i,
            None => {
                kids.push(json!({ "className": "Folder", "name": seg }));
                kids.len() - 1
            }
        };
        cur = kids.get_mut(idx)?;
    }
    if cur.get("children").is_none() {
        cur.as_object_mut()?.insert("children".into(), Value::Array(vec![]));
    }
    Some((cur.get_mut("children")?.as_array_mut()?, leaf))
}

/// Navigate to the mutable children Vec of the parent of `path`, WITHOUT
/// creating any missing ancestors. Returns None if any ancestor segment (or
/// the parent's `children` array) doesn't already exist. Returns
/// (parent_children, leaf_name) on success.
fn find_parent_children<'a>(root: &'a mut Value, path: &str) -> Option<(&'a mut Vec<Value>, String)> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }
    let leaf = segments[segments.len() - 1].to_string();
    let mut cur = root;
    for seg in &segments[..segments.len() - 1] {
        let kids = cur.get_mut("children")?.as_array_mut()?;
        let idx = kids.iter().position(|c| c.get("name").and_then(|n| n.as_str()) == Some(*seg))?;
        cur = kids.get_mut(idx)?;
    }
    Some((cur.get_mut("children")?.as_array_mut()?, leaf))
}

fn remove_at(root: &mut Value, path: &str) -> bool {
    if let Some((kids, leaf)) = find_parent_children(root, path) {
        if let Some(i) = kids.iter().position(|c| c.get("name").and_then(|n| n.as_str()) == Some(leaf.as_str())) {
            kids.remove(i);
            return true;
        }
    }
    false
}

fn upsert(root: &mut Value, path: &str, node: Value) -> bool {
    if let Some((kids, leaf)) = parent_children(root, path) {
        match kids.iter().position(|c| c.get("name").and_then(|n| n.as_str()) == Some(leaf.as_str())) {
            Some(i) => {
                // Preserve existing children when replacing an instance's own fields.
                let existing_children = kids[i].get("children").cloned();
                kids[i] = node;
                if let Some(ch) = existing_children {
                    kids[i].as_object_mut().unwrap().insert("children".into(), ch);
                }
            }
            None => kids.push(node),
        }
        return true;
    }
    false
}

/// Replace-or-push at `path` WITHOUT preserving any existing destination
/// node's children. Used for rename's destination insert, where the moved
/// node's own children (carried over from the source) are already correct
/// and must not be clobbered by a stale destination's children.
fn upsert_replace(root: &mut Value, path: &str, node: Value) -> bool {
    if let Some((kids, leaf)) = parent_children(root, path) {
        match kids.iter().position(|c| c.get("name").and_then(|n| n.as_str()) == Some(leaf.as_str())) {
            Some(i) => kids[i] = node,
            None => kids.push(node),
        }
        return true;
    }
    false
}

pub fn apply_ops(
    root: &mut Value,
    ops: &[SyncOp],
    src_dir_name: &str,
    tree_mapping: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;
    for op in ops {
        let ok = match op.op {
            OpKind::Delete => remove_at(root, &op.path),
            OpKind::Create | OpKind::Modify => {
                if let Some(data) = &op.data {
                    // data carries className/name/properties/... in the instance shape;
                    // ensure it has name/path for make_node.
                    let mut inst = data.clone();
                    let obj = inst.as_object_mut();
                    if let Some(obj) = obj {
                        obj.entry("path").or_insert(json!(op.path));
                        if !obj.contains_key("name") {
                            let nm = op.path.rsplit('/').next().unwrap_or("");
                            obj.insert("name".into(), json!(nm));
                        }
                    }
                    let node = make_node(&inst, src_dir_name, tree_mapping);
                    upsert(root, &op.path, node)
                } else {
                    false
                }
            }
            OpKind::Rename => {
                if let Some(new_path) = &op.new_path {
                    // Locate the existing subtree without synthesizing phantom
                    // ancestors if the source path doesn't actually exist.
                    if let Some((kids, leaf)) = find_parent_children(root, &op.path) {
                        if let Some(i) = kids.iter().position(|c| c.get("name").and_then(|n| n.as_str()) == Some(leaf.as_str())) {
                            let mut moved = kids.remove(i);
                            let new_name = new_path.rsplit('/').next().unwrap_or(leaf.as_str()).to_string();
                            moved.as_object_mut().unwrap().insert("name".into(), json!(new_name));
                            // Insert the moved node wholesale: it already carries its
                            // own (correct) children, so use the non-preserving upsert
                            // to avoid clobbering them with a stale destination's children.
                            upsert_replace(root, new_path, moved)
                        } else { false }
                    } else { false }
                } else { false }
            }
        };
        if ok { applied += 1; }
    }
    applied
}

#[cfg(test)]
mod apply_tests {
    use super::*;
    use serde_json::json;

    fn empty_root() -> Value { json!({ "className": "DataModel", "name": "DataModel", "children": [] }) }
    fn op(kind: OpKind, path: &str, data: Option<Value>, new_path: Option<&str>) -> SyncOp {
        SyncOp { op: kind, path: path.into(), new_path: new_path.map(|s| s.into()), data }
    }

    #[test]
    fn test_create_adds_node_under_synthesized_parents() {
        let mut root = empty_root();
        let n = apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Model/Part",
            Some(json!({"className":"Part","properties":{}})), None)], "src", &HashMap::new());
        assert_eq!(n, 1);
        let ws = root["children"].as_array().unwrap().iter().find(|c| c["name"]=="Workspace").unwrap();
        let model = ws["children"].as_array().unwrap().iter().find(|c| c["name"]=="Model").unwrap();
        assert_eq!(model["children"].as_array().unwrap()[0]["className"], "Part");
    }

    #[test]
    fn test_modify_updates_props_preserves_children() {
        let mut root = empty_root();
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace", Some(json!({"className":"Workspace","properties":{}})), None)], "src", &HashMap::new());
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Child", Some(json!({"className":"Folder","properties":{}})), None)], "src", &HashMap::new());
        apply_ops(&mut root, &[op(OpKind::Modify, "Workspace", Some(json!({"className":"Workspace","properties":{"Gravity":{"type":"float","value":10.0}}})), None)], "src", &HashMap::new());
        let ws = root["children"].as_array().unwrap().iter().find(|c| c["name"]=="Workspace").unwrap();
        assert_eq!(ws["properties"]["Gravity"], json!({"type":"float","value":10.0}));
        assert_eq!(ws["children"].as_array().unwrap().len(), 1, "child preserved through modify");
    }

    #[test]
    fn test_delete_removes_node() {
        let mut root = empty_root();
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Gone", Some(json!({"className":"Part","properties":{}})), None)], "src", &HashMap::new());
        let n = apply_ops(&mut root, &[op(OpKind::Delete, "Workspace/Gone", None, None)], "src", &HashMap::new());
        assert_eq!(n, 1);
        let ws = root["children"].as_array().unwrap().iter().find(|c| c["name"]=="Workspace").unwrap();
        assert!(ws["children"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_rename_moves_subtree() {
        let mut root = empty_root();
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Old", Some(json!({"className":"Model","properties":{}})), None)], "src", &HashMap::new());
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Old/Leaf", Some(json!({"className":"Part","properties":{}})), None)], "src", &HashMap::new());
        let n = apply_ops(&mut root, &[op(OpKind::Rename, "Workspace/Old", None, Some("Workspace/New"))], "src", &HashMap::new());
        assert_eq!(n, 1);
        let ws = root["children"].as_array().unwrap().iter().find(|c| c["name"]=="Workspace").unwrap();
        let kids = ws["children"].as_array().unwrap();
        assert!(kids.iter().all(|c| c["name"] != "Old"));
        let new = kids.iter().find(|c| c["name"]=="New").unwrap();
        assert_eq!(new["children"].as_array().unwrap()[0]["name"], "Leaf");
    }

    #[test]
    fn test_unresolvable_op_skipped() {
        let mut root = empty_root();
        let n = apply_ops(&mut root, &[op(OpKind::Delete, "", None, None)], "src", &HashMap::new());
        assert_eq!(n, 0);
    }

    #[test]
    fn test_rename_onto_existing_target_keeps_moved_childrens() {
        let mut root = empty_root();
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Src", Some(json!({"className":"Model","properties":{}})), None)], "src", &HashMap::new());
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Src/Kept", Some(json!({"className":"Part","properties":{}})), None)], "src", &HashMap::new());
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Dst", Some(json!({"className":"Model","properties":{}})), None)], "src", &HashMap::new());
        apply_ops(&mut root, &[op(OpKind::Create, "Workspace/Dst/Stale", Some(json!({"className":"Part","properties":{}})), None)], "src", &HashMap::new());
        // rename Src -> Dst (collision): Dst must end up with Src's child "Kept", not the stale "Stale"
        apply_ops(&mut root, &[op(OpKind::Rename, "Workspace/Src", None, Some("Workspace/Dst"))], "src", &HashMap::new());
        let ws = root["children"].as_array().unwrap().iter().find(|c| c["name"]=="Workspace").unwrap();
        let dst = ws["children"].as_array().unwrap().iter().find(|c| c["name"]=="Dst").unwrap();
        let kids: Vec<&str> = dst["children"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(kids, vec!["Kept"], "moved node keeps its own child, not the stale destination's");
    }

    #[test]
    fn test_delete_nonexistent_path_no_phantom_folders() {
        let mut root = empty_root();
        let n = apply_ops(&mut root, &[op(OpKind::Delete, "Workspace/Ghost/Deep", None, None)], "src", &HashMap::new());
        assert_eq!(n, 0);
        assert!(root["children"].as_array().unwrap().is_empty(), "no phantom Workspace/Ghost folders created");
    }

    #[test]
    fn test_rename_nonexistent_source_no_phantom_folders() {
        let mut root = empty_root();
        let n = apply_ops(&mut root, &[op(OpKind::Rename, "Nowhere/Old", None, Some("Nowhere/New"))], "src", &HashMap::new());
        assert_eq!(n, 0);
        assert!(root["children"].as_array().unwrap().is_empty(), "no phantom folders from a failed rename source lookup");
    }
}
