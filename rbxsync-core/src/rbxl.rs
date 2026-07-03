//! Convert a parsed Roblox place `WeakDom` into the `.rbxjson` instance array
//! (the same shape the Studio plugin streams), for building a baseline snapshot.

use rbx_dom_weak::types::{Ref, Variant};
use rbx_dom_weak::WeakDom;
use serde_json::{json, Value};

use crate::variant_json::variant_to_json;

/// Walk the DOM depth-first, emitting one instance object per node under the
/// DataModel root. Paths are DataModel paths ("Workspace/Model/Part").
pub fn dom_to_instances(dom: &WeakDom) -> Vec<Value> {
    let mut out = Vec::new();
    for &child in dom.root().children() {
        walk(dom, child, "", &mut out);
    }
    out
}

fn walk(dom: &WeakDom, inst_ref: Ref, parent_path: &str, out: &mut Vec<Value>) {
    let Some(instance) = dom.get_by_ref(inst_ref) else { return };
    let name = instance.name.clone();
    let escaped_name = name.replace('/', "[SLASH]");
    let path = if parent_path.is_empty() {
        escaped_name
    } else {
        format!("{}/{}", parent_path, escaped_name)
    };

    let mut properties = serde_json::Map::new();
    let mut attributes = serde_json::Map::new();
    let mut tags: Vec<Value> = Vec::new();

    for (prop_name, variant) in instance.properties.iter() {
        let prop_name = prop_name.to_string();
        match variant {
            Variant::Attributes(attrs) => {
                for (k, v) in attrs.iter() {
                    if let Some(encoded) = variant_to_json(v) {
                        attributes.insert(k.to_string(), encoded);
                    }
                }
            }
            Variant::Tags(t) => {
                for tag in t.iter() {
                    tags.push(json!(tag));
                }
            }
            other => {
                if let Some(encoded) = variant_to_json(other) {
                    properties.insert(prop_name, encoded);
                }
            }
        }
    }

    let mut obj = json!({
        "className": instance.class,
        "name": name,
        "path": path,
        "referenceId": inst_ref.to_string(),
        "properties": properties,
    });
    let map = obj.as_object_mut().expect("json!({...}) always builds an object");
    if !attributes.is_empty() {
        map.insert("attributes".to_string(), Value::Object(attributes));
    }
    if !tags.is_empty() {
        map.insert("tags".to_string(), Value::Array(tags));
    }
    out.push(obj);

    for &child in instance.children() {
        walk(dom, child, &path, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rbx_dom_weak::types::Variant;
    use rbx_dom_weak::InstanceBuilder;

    #[test]
    fn test_walk_nested_tree_with_script_and_props() {
        let part = InstanceBuilder::new("Part")
            .with_name("Block")
            .with_property("Anchored", Variant::Bool(true));
        let script = InstanceBuilder::new("Script")
            .with_name("Main")
            .with_property("Source", Variant::String("print('hi')".into()));
        let folder = InstanceBuilder::new("Folder").with_name("Stuff").with_children([part, script]);
        let workspace = InstanceBuilder::new("Workspace").with_name("Workspace").with_child(folder);
        let dom = WeakDom::new(InstanceBuilder::new("DataModel").with_child(workspace));

        let instances = dom_to_instances(&dom);
        let by_path = |p: &str| instances.iter().find(|i| i["path"] == p)
            .unwrap_or_else(|| panic!("no {p}: {instances:#?}"));

        assert_eq!(by_path("Workspace")["className"], "Workspace");
        assert_eq!(by_path("Workspace/Stuff")["className"], "Folder");
        let block = by_path("Workspace/Stuff/Block");
        assert_eq!(block["className"], "Part");
        assert_eq!(block["properties"]["Anchored"], json!({"type":"bool","value":true}));
        let main = by_path("Workspace/Stuff/Main");
        assert_eq!(main["className"], "Script");
        assert_eq!(main["properties"]["Source"]["value"], "print('hi')");
    }

    #[test]
    fn test_attributes_and_tags_lifted_out() {
        use rbx_dom_weak::types::{Attributes, Tags};
        let mut attrs = Attributes::new();
        attrs.insert("Level".into(), Variant::Int32(3));
        let part = InstanceBuilder::new("Part")
            .with_name("P")
            .with_property("Attributes", Variant::Attributes(attrs))
            .with_property("Tags", Variant::Tags(Tags::from(vec!["Spawn".to_string()])));
        let dom = WeakDom::new(InstanceBuilder::new("DataModel")
            .with_child(InstanceBuilder::new("Workspace").with_name("Workspace").with_child(part)));

        let instances = dom_to_instances(&dom);
        let p = instances.iter().find(|i| i["path"] == "Workspace/P").unwrap();
        assert_eq!(p["attributes"]["Level"], json!({"type":"int32","value":3}));
        assert_eq!(p["tags"], json!(["Spawn"]));
    }

    #[test]
    fn test_duplicate_named_siblings_get_distinct_reference_ids() {
        use rbx_dom_weak::InstanceBuilder;
        let dom = WeakDom::new(InstanceBuilder::new("DataModel").with_child(
            InstanceBuilder::new("Workspace").with_name("Workspace").with_children([
                InstanceBuilder::new("Part").with_name("Part"),
                InstanceBuilder::new("Part").with_name("Part"),
                InstanceBuilder::new("Part").with_name("Part"),
            ])));
        let instances = dom_to_instances(&dom);
        let ids: std::collections::HashSet<&str> = instances.iter()
            .filter(|i| i["path"] == "Workspace/Part")
            .map(|i| i["referenceId"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), 3, "three same-named siblings must have three distinct referenceIds");
        // And the 8-char disambiguation prefixes must differ
        let prefixes: std::collections::HashSet<String> = ids.iter().map(|id| id[..8].to_string()).collect();
        assert_eq!(prefixes.len(), 3, "8-char disambiguation prefixes must be distinct");
    }

    #[test]
    fn test_slash_in_name_is_escaped_in_path_but_not_name() {
        let part = InstanceBuilder::new("Part").with_name("a/b");
        let dom = WeakDom::new(InstanceBuilder::new("DataModel")
            .with_child(InstanceBuilder::new("Workspace").with_name("Workspace").with_child(part)));

        let instances = dom_to_instances(&dom);
        let p = instances.iter().find(|i| i["name"] == "a/b").unwrap();
        assert_eq!(p["path"], "Workspace/a[SLASH]b");
    }
}
