import * as vscode from 'vscode';
import * as fs from 'fs';

/**
 * Cache for ClassName lookups from .rbxjson files
 */
const classNameCache = new Map<string, string>();

/**
 * Map of Roblox class names to their badge colors
 */
const classColors: Record<string, vscode.ThemeColor> = {
  // Scripts
  Script: new vscode.ThemeColor('charts.blue'),
  LocalScript: new vscode.ThemeColor('charts.blue'),
  ModuleScript: new vscode.ThemeColor('charts.purple'),

  // Instances
  Part: new vscode.ThemeColor('charts.gray'),
  MeshPart: new vscode.ThemeColor('charts.green'),
  UnionOperation: new vscode.ThemeColor('charts.blue'),
  Model: new vscode.ThemeColor('charts.gray'),
  Folder: new vscode.ThemeColor('charts.yellow'),

  // Events
  RemoteEvent: new vscode.ThemeColor('charts.red'),
  RemoteFunction: new vscode.ThemeColor('charts.purple'),
  BindableEvent: new vscode.ThemeColor('charts.yellow'),
  BindableFunction: new vscode.ThemeColor('charts.orange'),

  // Values
  StringValue: new vscode.ThemeColor('charts.green'),
  NumberValue: new vscode.ThemeColor('charts.orange'),
  BoolValue: new vscode.ThemeColor('charts.purple'),
  IntValue: new vscode.ThemeColor('charts.orange'),
  ObjectValue: new vscode.ThemeColor('charts.gray'),

  // Other
  Configuration: new vscode.ThemeColor('charts.yellow'),
  Sound: new vscode.ThemeColor('charts.blue'),
  SpawnLocation: new vscode.ThemeColor('charts.green'),
  Camera: new vscode.ThemeColor('charts.green'),
  Workspace: new vscode.ThemeColor('charts.blue'),
};

/**
 * Get abbreviated class name for badge display
 */
function getClassBadge(className: string): string {
  // Return first 2-3 characters of class name for badge
  const abbreviations: Record<string, string> = {
    Script: 'S',
    LocalScript: 'LS',
    ModuleScript: 'MS',
    Part: 'P',
    MeshPart: 'MP',
    UnionOperation: 'U',
    Model: 'M',
    Folder: 'F',
    RemoteEvent: 'RE',
    RemoteFunction: 'RF',
    BindableEvent: 'BE',
    BindableFunction: 'BF',
    StringValue: 'Str',
    NumberValue: 'Num',
    BoolValue: 'Bool',
    IntValue: 'Int',
    ObjectValue: 'Obj',
    Configuration: 'Cfg',
    Sound: 'Snd',
    SpawnLocation: 'SP',
    Camera: 'Cam',
    Workspace: 'WS',
  };

  return abbreviations[className] || className.substring(0, 2);
}

/**
 * Extract ClassName from a .rbxjson file
 * Reads only the first part of the file for performance
 */
function extractClassName(filePath: string): string | undefined {
  // Check cache first
  if (classNameCache.has(filePath)) {
    return classNameCache.get(filePath);
  }

  try {
    // Read only first 500 bytes for performance
    const fd = fs.openSync(filePath, 'r');
    const buffer = Buffer.alloc(500);
    fs.readSync(fd, buffer, 0, 500, 0);
    fs.closeSync(fd);

    const content = buffer.toString('utf8');

    // Quick regex to find ClassName
    const match = content.match(/"ClassName"\s*:\s*"([^"]+)"/);
    if (match) {
      const className = match[1];
      classNameCache.set(filePath, className);
      return className;
    }
  } catch {
    // File doesn't exist or can't be read
  }

  return undefined;
}

/**
 * FileDecorationProvider for .rbxjson files
 * Shows the Roblox class type as a badge
 */
export class RbxJsonDecorationProvider implements vscode.FileDecorationProvider {
  private readonly _onDidChangeFileDecorations = new vscode.EventEmitter<vscode.Uri | vscode.Uri[] | undefined>();
  readonly onDidChangeFileDecorations = this._onDidChangeFileDecorations.event;

  private readonly fileWatcher: vscode.FileSystemWatcher;

  constructor() {
    // Watch for changes to .rbxjson files
    this.fileWatcher = vscode.workspace.createFileSystemWatcher('**/*.rbxjson');

    this.fileWatcher.onDidChange(uri => {
      // Clear cache and refresh
      classNameCache.delete(uri.fsPath);
      this._onDidChangeFileDecorations.fire(uri);
    });

    this.fileWatcher.onDidCreate(uri => {
      this._onDidChangeFileDecorations.fire(uri);
    });

    this.fileWatcher.onDidDelete(uri => {
      classNameCache.delete(uri.fsPath);
      this._onDidChangeFileDecorations.fire(uri);
    });
  }

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    // Only handle .rbxjson files
    if (!uri.fsPath.endsWith('.rbxjson')) {
      return undefined;
    }

    const className = extractClassName(uri.fsPath);
    if (!className) {
      return undefined;
    }

    const badge = getClassBadge(className);
    const color = classColors[className];

    return {
      badge,
      tooltip: `Roblox ${className}`,
      color,
    };
  }

  dispose(): void {
    this.fileWatcher.dispose();
    this._onDidChangeFileDecorations.dispose();
  }
}

/**
 * Clear the class name cache
 * Called when workspace changes
 */
export function clearClassNameCache(): void {
  classNameCache.clear();
}
