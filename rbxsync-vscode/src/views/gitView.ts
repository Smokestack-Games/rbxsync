import * as vscode from 'vscode';
import { GitStatusResponse } from '../server/types';

type GitFileItem = { path: string; type: 'staged' | 'modified' | 'untracked' };
type GitItem = { type: 'category'; label: string; files: GitFileItem[] } | GitFileItem;

export class GitTreeProvider implements vscode.TreeDataProvider<GitItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<GitItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private gitStatus: GitStatusResponse | null = null;
  private projectDir: string | null = null;

  setProjectDir(dir: string): void {
    this.projectDir = dir;
  }

  updateStatus(status: GitStatusResponse): void {
    this.gitStatus = status;
    this._onDidChangeTreeData.fire(undefined);
  }

  getTreeItem(element: GitItem): vscode.TreeItem {
    if (element.type === 'category') {
      const item = new vscode.TreeItem(
        element.label,
        element.files.length > 0
          ? vscode.TreeItemCollapsibleState.Expanded
          : vscode.TreeItemCollapsibleState.None
      );
      item.description = `${element.files.length} files`;
      return item;
    }

    // File item
    const item = new vscode.TreeItem(element.path, vscode.TreeItemCollapsibleState.None);
    item.iconPath = this.getIconForFileType(element.type);
    item.contextValue = element.type;

    return item;
  }

  getChildren(element?: GitItem): GitItem[] {
    if (!this.gitStatus || !this.gitStatus.is_repo) {
      return [];
    }

    if (!element) {
      // Root level - return categories
      const categories: GitItem[] = [];

      if (this.gitStatus.staged.length > 0) {
        categories.push({
          type: 'category',
          label: 'Staged Changes',
          files: this.gitStatus.staged.map(p => ({ path: p, type: 'staged' as const }))
        });
      }

      if (this.gitStatus.modified.length > 0) {
        categories.push({
          type: 'category',
          label: 'Modified',
          files: this.gitStatus.modified.map(p => ({ path: p, type: 'modified' as const }))
        });
      }

      if (this.gitStatus.untracked.length > 0) {
        categories.push({
          type: 'category',
          label: 'Untracked',
          files: this.gitStatus.untracked.map(p => ({ path: p, type: 'untracked' as const }))
        });
      }

      return categories;
    }

    if (element.type === 'category') {
      return element.files;
    }

    return [];
  }

  private getIconForFileType(type: string): vscode.ThemeIcon {
    switch (type) {
      case 'staged':
        return new vscode.ThemeIcon('diff-added', new vscode.ThemeColor('gitDecoration.addedResourceForeground'));
      case 'modified':
        return new vscode.ThemeIcon('diff-modified', new vscode.ThemeColor('gitDecoration.modifiedResourceForeground'));
      case 'untracked':
        return new vscode.ThemeIcon('diff-ignored', new vscode.ThemeColor('gitDecoration.untrackedResourceForeground'));
      default:
        return new vscode.ThemeIcon('file');
    }
  }

  getBranchInfo(): string | null {
    if (!this.gitStatus || !this.gitStatus.is_repo) return null;

    let info = this.gitStatus.branch || 'unknown';
    if (this.gitStatus.ahead && this.gitStatus.ahead > 0) {
      info += ` ↑${this.gitStatus.ahead}`;
    }
    if (this.gitStatus.behind && this.gitStatus.behind > 0) {
      info += ` ↓${this.gitStatus.behind}`;
    }
    return info;
  }

  dispose(): void {
    this._onDidChangeTreeData.dispose();
  }
}
