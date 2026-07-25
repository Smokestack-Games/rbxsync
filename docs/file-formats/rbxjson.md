# The Context File

`datamodel.rbxjson` is a single JSON document at the project root that holds the
whole instance tree. RbxSync writes it during extraction and keeps it up to date
as instances change in Studio.

## Basic Structure

The document is one nested tree rooted at the DataModel. Each instance is a node;
its children are nested under a `children` array.

```json
{
  "className": "DataModel",
  "name": "DataModel",
  "children": [
    {
      "className": "Workspace",
      "name": "Workspace",
      "children": [
        {
          "className": "Part",
          "name": "Baseplate",
          "properties": {
            "Anchored": { "type": "bool", "value": true },
            "Size": { "type": "Vector3", "value": { "x": 512, "y": 20, "z": 512 } }
          }
        }
      ]
    }
  ]
}
```

## Node Fields

| Field | Present when | Description |
|-------|--------------|-------------|
| `className` | Always | Roblox class name |
| `name` | Always | Instance name |
| `properties` | Has non-default properties | Property values (see below) |
| `attributes` | Has attributes | Instance attributes |
| `tags` | Has CollectionService tags | Array of tag strings |
| `children` | Has children | Nested instance nodes |
| `sourcePath` | Scripts only | Path to the `.luau` file holding the source |

Empty `properties`, `attributes`, `tags`, and `children` are omitted rather than
written as empty objects or arrays.

## Property Format

Each property has an explicit `type` and `value`:

```json
"PropertyName": {
  "type": "TypeName",
  "value": <value>
}
```

Explicit types keep round-trips lossless — types that look alike (for example
`Color3` vs `Color3uint8`) serialize differently. See [Property Types](/file-formats/property-types)
for the full list.

## Scripts in the Tree

A script instance appears as a node like any other, but its `Source` property is
removed and replaced by a `sourcePath` pointing at the `.luau` file that holds
the code. Any other script properties (such as `Enabled`) stay on the node.

```json
{
  "className": "Script",
  "name": "Main",
  "properties": {
    "Enabled": { "type": "bool", "value": true }
  },
  "sourcePath": "src/ServerScriptService/Main.server.luau"
}
```

See [Scripts on Disk](/file-formats/luau) for the `.luau` side of this mapping.

## Duplicate Siblings

Instances that share a name and parent are all kept as distinct nodes. Their
`name` fields stay identical; RbxSync disambiguates them internally (and in the
`sourcePath` filenames of duplicate scripts) so none are lost.

## Editing

`datamodel.rbxjson` is generated and maintained by RbxSync — don't hand-edit it.
It exists to give AI assistants and the Luau language server a full view of the
game. The sync-to-Studio path reads scripts only, so editing this file will not
push non-script changes to Studio. To change a non-script instance, edit it in
Studio and let the change flow back into the context file.
