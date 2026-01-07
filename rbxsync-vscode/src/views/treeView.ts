import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

interface TreeNode {
  name: string;
  className: string;
  path: string;
  filePath?: string;
  children: Map<string, TreeNode>;
}

export class InstanceTreeProvider implements vscode.TreeDataProvider<TreeNode> {
  private _onDidChangeTreeData = new vscode.EventEmitter<TreeNode | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private rootNodes: Map<string, TreeNode> = new Map();
  private projectDir: string | null = null;

  constructor() {}

  setProjectDir(dir: string): void {
    this.projectDir = dir;
    this.refresh();
  }

  refresh(): void {
    if (this.projectDir) {
      this.loadFromFileSystem();
    }
    this._onDidChangeTreeData.fire(undefined);
  }

  private loadFromFileSystem(): void {
    if (!this.projectDir) return;

    this.rootNodes.clear();
    const srcDir = path.join(this.projectDir, 'src');

    if (!fs.existsSync(srcDir)) {
      return;
    }

    const entries = fs.readdirSync(srcDir, { withFileTypes: true });

    // Group entries: find directories and their matching .rbxjson files
    const directories = entries.filter(e => e.isDirectory() && !e.name.startsWith('.'));
    const jsonFiles = new Map<string, string>();

    for (const entry of entries) {
      if (entry.isFile() && entry.name.endsWith('.rbxjson')) {
        const baseName = entry.name.replace('.rbxjson', '');
        jsonFiles.set(baseName, path.join(srcDir, entry.name));
      }
    }

    // Process each service directory
    for (const dir of directories) {
      const dirPath = path.join(srcDir, dir.name);
      const metaPath = jsonFiles.get(dir.name) || path.join(dirPath, '_meta.rbxjson');

      let className = this.getDefaultServiceClass(dir.name);
      let name = dir.name;

      // Try to read metadata from .rbxjson file
      if (fs.existsSync(metaPath)) {
        try {
          const meta = JSON.parse(fs.readFileSync(metaPath, 'utf-8'));
          className = meta.className || className;
          name = meta.name || name;
        } catch {}
      }

      const node: TreeNode = {
        name,
        className,
        path: dir.name,
        filePath: dirPath,
        children: new Map()
      };

      // Load children from the directory
      this.loadDirectoryContents(dirPath, dir.name, node);
      this.rootNodes.set(dir.name, node);
    }

    // Also check for standalone .rbxjson files (services without folders)
    for (const [baseName, jsonPath] of jsonFiles) {
      if (!this.rootNodes.has(baseName)) {
        try {
          const data = JSON.parse(fs.readFileSync(jsonPath, 'utf-8'));
          this.rootNodes.set(baseName, {
            name: data.name || baseName,
            className: data.className || this.getDefaultServiceClass(baseName),
            path: baseName,
            filePath: jsonPath,
            children: new Map()
          });
        } catch {}
      }
    }
  }

  private loadDirectoryContents(dirPath: string, instancePath: string, parentNode: TreeNode): void {
    if (!fs.existsSync(dirPath)) return;

    const entries = fs.readdirSync(dirPath, { withFileTypes: true });
    const directories = entries.filter(e => e.isDirectory() && !e.name.startsWith('.'));
    const files = entries.filter(e => e.isFile() && !e.name.startsWith('.'));

    // Create a map for .rbxjson files
    const jsonFiles = new Map<string, string>();
    for (const file of files) {
      if (file.name.endsWith('.rbxjson') && file.name !== '_meta.rbxjson') {
        const baseName = file.name.replace('.rbxjson', '');
        jsonFiles.set(baseName, path.join(dirPath, file.name));
      }
    }

    // Process subdirectories
    for (const dir of directories) {
      const childDirPath = path.join(dirPath, dir.name);
      const childPath = `${instancePath}.${dir.name}`;
      const metaJsonPath = jsonFiles.get(dir.name) || path.join(childDirPath, '_meta.rbxjson');

      let className = 'Folder';
      let name = dir.name;

      if (fs.existsSync(metaJsonPath)) {
        try {
          const meta = JSON.parse(fs.readFileSync(metaJsonPath, 'utf-8'));
          className = meta.className || className;
          name = meta.name || name;
        } catch {}
      }

      const childNode: TreeNode = {
        name,
        className,
        path: childPath,
        filePath: childDirPath,
        children: new Map()
      };

      this.loadDirectoryContents(childDirPath, childPath, childNode);
      parentNode.children.set(dir.name, childNode);

      // Remove from jsonFiles so we don't add it again
      jsonFiles.delete(dir.name);
    }

    // Process script files (.luau, .lua)
    for (const file of files) {
      if (file.name.endsWith('.luau') || file.name.endsWith('.lua')) {
        const filePath = path.join(dirPath, file.name);
        const baseName = file.name
          .replace('.server.luau', '')
          .replace('.client.luau', '')
          .replace('.luau', '')
          .replace('.server.lua', '')
          .replace('.client.lua', '')
          .replace('.lua', '');

        let className = 'ModuleScript';
        if (file.name.includes('.server.')) className = 'Script';
        else if (file.name.includes('.client.')) className = 'LocalScript';

        // Check if there's a matching .rbxjson with more info
        const jsonPath = jsonFiles.get(baseName);
        if (jsonPath) {
          try {
            const data = JSON.parse(fs.readFileSync(jsonPath, 'utf-8'));
            className = data.className || className;
          } catch {}
          jsonFiles.delete(baseName);
        }

        if (!parentNode.children.has(baseName)) {
          parentNode.children.set(baseName, {
            name: baseName,
            className,
            path: `${instancePath}.${baseName}`,
            filePath,
            children: new Map()
          });
        }
      }
    }

    // Process remaining standalone .rbxjson files
    for (const [baseName, jsonPath] of jsonFiles) {
      if (baseName === '_meta') continue;
      if (!parentNode.children.has(baseName)) {
        try {
          const data = JSON.parse(fs.readFileSync(jsonPath, 'utf-8'));
          parentNode.children.set(baseName, {
            name: data.name || baseName,
            className: data.className || 'Instance',
            path: `${instancePath}.${baseName}`,
            filePath: jsonPath,
            children: new Map()
          });
        } catch {}
      }
    }
  }

  private getDefaultServiceClass(name: string): string {
    const serviceClasses: Record<string, string> = {
      'Workspace': 'Workspace',
      'ServerScriptService': 'ServerScriptService',
      'ReplicatedStorage': 'ReplicatedStorage',
      'ReplicatedFirst': 'ReplicatedFirst',
      'StarterGui': 'StarterGui',
      'StarterPack': 'StarterPack',
      'StarterPlayer': 'StarterPlayer',
      'ServerStorage': 'ServerStorage',
      'Lighting': 'Lighting',
      'SoundService': 'SoundService',
      'Chat': 'Chat',
      'Teams': 'Teams',
      'LocalizationService': 'LocalizationService',
      'TestService': 'TestService'
    };
    return serviceClasses[name] || 'Folder';
  }

  getTreeItem(element: TreeNode): vscode.TreeItem {
    const hasChildren = element.children.size > 0;
    const item = new vscode.TreeItem(
      element.name,
      hasChildren
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None
    );

    item.description = element.className;
    item.iconPath = this.getIconForClass(element.className);

    // Use MarkdownString for better tooltip rendering
    const tooltip = new vscode.MarkdownString();
    tooltip.appendMarkdown(`**${element.name}**\n\n`);
    tooltip.appendMarkdown(`Class: \`${element.className}\`\n\n`);
    tooltip.appendMarkdown(`Path: \`${element.path}\``);
    if (element.filePath) {
      tooltip.appendMarkdown(`\n\nFile: \`${element.filePath}\``);
    }
    item.tooltip = tooltip;

    item.contextValue = 'instance';

    if (element.filePath) {
      item.command = {
        command: 'rbxsync.openFile',
        title: 'Open File',
        arguments: [element]
      };
    }

    return item;
  }

  getChildren(element?: TreeNode): TreeNode[] {
    if (!element) {
      // Root level - return services sorted by standard order
      return Array.from(this.rootNodes.values())
        .sort((a, b) => this.getServiceOrder(a.name) - this.getServiceOrder(b.name));
    }
    return Array.from(element.children.values())
      .sort((a, b) => {
        // Sort scripts first, then folders, then by name
        const aIsScript = ['Script', 'LocalScript', 'ModuleScript'].includes(a.className);
        const bIsScript = ['Script', 'LocalScript', 'ModuleScript'].includes(b.className);
        if (aIsScript && !bIsScript) return -1;
        if (!aIsScript && bIsScript) return 1;
        return a.name.localeCompare(b.name);
      });
  }

  private getServiceOrder(name: string): number {
    const order: Record<string, number> = {
      'Workspace': 0,
      'ServerScriptService': 1,
      'ReplicatedStorage': 2,
      'ReplicatedFirst': 3,
      'StarterGui': 4,
      'StarterPack': 5,
      'StarterPlayer': 6,
      'ServerStorage': 7,
      'Lighting': 8,
      'SoundService': 9,
      'Chat': 10,
      'Teams': 11,
      'LocalizationService': 12
    };
    return order[name] ?? 99;
  }

  private getIconForClass(className: string): vscode.ThemeIcon {
    const iconMap: Record<string, string> = {
      'Script': 'file-code',
      'LocalScript': 'file-code',
      'ModuleScript': 'file-code',
      'Part': 'primitive-square',
      'MeshPart': 'primitive-square',
      'Model': 'symbol-class',
      'Folder': 'folder',
      'Frame': 'symbol-interface',
      'TextLabel': 'symbol-text',
      'TextButton': 'symbol-event',
      'ImageLabel': 'file-media',
      'Camera': 'eye',
      'Sound': 'unmute',
      'PointLight': 'lightbulb',
      'SpotLight': 'lightbulb',
      'SurfaceLight': 'lightbulb',
      'Workspace': 'globe',
      'ServerScriptService': 'server',
      'ReplicatedStorage': 'database',
      'ReplicatedFirst': 'zap',
      'StarterGui': 'layout',
      'StarterPack': 'package',
      'StarterPlayer': 'person',
      'ServerStorage': 'archive',
      'Lighting': 'lightbulb',
      'SoundService': 'unmute',
      'Teams': 'organization',
      'Chat': 'comment',
      'LocalizationService': 'globe'
    };
    return new vscode.ThemeIcon(iconMap[className] || 'symbol-misc');
  }

  getNodeByPath(instancePath: string): TreeNode | undefined {
    const parts = instancePath.split('.');
    if (parts.length === 0) return undefined;

    let current = this.rootNodes.get(parts[0]);
    for (let i = 1; i < parts.length && current; i++) {
      current = current.children.get(parts[i]);
    }
    return current;
  }

  dispose(): void {
    this._onDidChangeTreeData.dispose();
  }
}
