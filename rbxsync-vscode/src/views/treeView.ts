import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { InstanceData } from '../server/types';

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

    // Load services as root nodes
    const services = fs.readdirSync(srcDir, { withFileTypes: true })
      .filter(d => d.isDirectory());

    for (const service of services) {
      const serviceNode = this.loadDirectory(path.join(srcDir, service.name), service.name);
      if (serviceNode) {
        this.rootNodes.set(service.name, serviceNode);
      }
    }
  }

  private loadDirectory(dirPath: string, instancePath: string): TreeNode | null {
    if (!fs.existsSync(dirPath)) return null;

    // Check for _meta.rbxjson
    const metaPath = path.join(dirPath, '_meta.rbxjson');
    let className = 'Folder';
    let name = path.basename(dirPath);

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
      path: instancePath,
      filePath: dirPath,
      children: new Map()
    };

    // Load children
    const entries = fs.readdirSync(dirPath, { withFileTypes: true });

    for (const entry of entries) {
      if (entry.name === '_meta.rbxjson') continue;

      const childPath = path.join(dirPath, entry.name);

      if (entry.isDirectory()) {
        const childNode = this.loadDirectory(childPath, `${instancePath}.${entry.name}`);
        if (childNode) {
          node.children.set(entry.name, childNode);
        }
      } else if (entry.name.endsWith('.rbxjson')) {
        // Instance file
        const baseName = entry.name.replace('.rbxjson', '');
        try {
          const data = JSON.parse(fs.readFileSync(childPath, 'utf-8'));
          node.children.set(baseName, {
            name: data.name || baseName,
            className: data.className || 'Instance',
            path: `${instancePath}.${baseName}`,
            filePath: childPath,
            children: new Map()
          });
        } catch {}
      } else if (entry.name.endsWith('.luau') || entry.name.endsWith('.lua')) {
        // Script file - check for accompanying .rbxjson
        const baseName = entry.name
          .replace('.server.luau', '')
          .replace('.client.luau', '')
          .replace('.luau', '')
          .replace('.server.lua', '')
          .replace('.client.lua', '')
          .replace('.lua', '');

        let className = 'ModuleScript';
        if (entry.name.includes('.server.')) className = 'Script';
        else if (entry.name.includes('.client.')) className = 'LocalScript';

        if (!node.children.has(baseName)) {
          node.children.set(baseName, {
            name: baseName,
            className,
            path: `${instancePath}.${baseName}`,
            filePath: childPath,
            children: new Map()
          });
        }
      }
    }

    return node;
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
    item.tooltip = element.path;
    item.contextValue = 'instance';

    if (element.filePath && !hasChildren) {
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
      // Root level - return services
      return Array.from(this.rootNodes.values())
        .sort((a, b) => this.getServiceOrder(a.name) - this.getServiceOrder(b.name));
    }
    return Array.from(element.children.values())
      .sort((a, b) => a.name.localeCompare(b.name));
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
      'Teams': 11
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
      'Light': 'lightbulb',
      'Workspace': 'globe',
      'ServerScriptService': 'server',
      'ReplicatedStorage': 'database',
      'StarterGui': 'layout',
      'StarterPack': 'package',
      'Lighting': 'lightbulb',
      'Teams': 'organization'
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
