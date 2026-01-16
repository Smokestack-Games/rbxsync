# Luau LSP Integration

RbxSync automatically generates configuration files for [Luau LSP](https://github.com/JohnnyMorganz/luau-lsp), giving you full intellisense, type checking, and autocomplete in VS Code.

## Automatic Setup

When you extract a game, RbxSync creates a `project.json` file in your project root:

```json
{
  "name": "YourGame",
  "tree": {
    "$path": "src",
    "Workspace": { "$path": "src/Workspace" },
    "ReplicatedStorage": { "$path": "src/ReplicatedStorage" },
    "ServerScriptService": { "$path": "src/ServerScriptService" }
  }
}
```

This file tells Luau LSP how your folder structure maps to Roblox services.

## Features You Get

With Luau LSP configured:

- **Autocomplete**: Full intellisense for Roblox APIs and your own modules
- **Type Checking**: Catch type errors before testing in Studio
- **Go to Definition**: Jump to module definitions across your codebase
- **Hover Info**: See type signatures and documentation on hover
- **Find References**: Find all usages of a function or variable

## Installing Luau LSP

1. Install the [Luau LSP extension](https://marketplace.visualstudio.com/items?itemName=JohnnyMorganz.luau-lsp) in VS Code
2. Extract your game with RbxSync (creates `project.json` automatically)
3. Open your project folder in VS Code
4. Luau LSP will detect the project and start providing intellisense

## Manual Sourcemap Generation

If you need to regenerate the sourcemap manually:

```bash
rbxsync sourcemap
```

This creates `sourcemap.json` which provides additional path resolution for the LSP.

## Troubleshooting

### LSP Not Working

1. Check that `project.json` exists in your project root
2. Verify the Luau LSP extension is installed and enabled
3. Reload VS Code window (`Cmd+Shift+P` → "Reload Window")

### Wrong Service Paths

If autocomplete suggests wrong paths, your `project.json` may be out of date. Re-extract or run:

```bash
rbxsync sourcemap --regenerate
```

### Rojo Compatibility

If you have an existing `default.project.json` from Rojo, Luau LSP will use that instead. RbxSync's `project.json` uses the same format, so both tools work together.

## VS Code Settings

For the best experience, add to your `.vscode/settings.json`:

```json
{
  "luau-lsp.sourcemap.enabled": true,
  "luau-lsp.types.roblox": true
}
```
