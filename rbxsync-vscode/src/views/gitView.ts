import * as vscode from 'vscode';
import * as cp from 'child_process';
import { GitStatusResponse } from '../server/types';

type GitFileItem = { path: string; type: 'staged' | 'modified' | 'untracked' };
type GitStatusItem = { type: 'status'; label: string; description?: string };
type GitItem = { type: 'category'; label: string; files: GitFileItem[] } | GitFileItem | GitStatusItem;

export class GitTreeProvider implements vscode.TreeDataProvider<GitItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<GitItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private gitStatus: GitStatusResponse | null = null;
  private projectDir: string | null = null;

  setProjectDir(dir: string): void {
    this.projectDir = dir;
    // Auto-refresh when project dir is set
    this.refreshLocalStatus();
  }

  updateStatus(status: GitStatusResponse): void {
    this.gitStatus = status;
    this._onDidChangeTreeData.fire(undefined);
  }

  async refreshLocalStatus(): Promise<void> {
    if (!this.projectDir) return;

    try {
      const status = await this.getLocalGitStatus(this.projectDir);
      if (status) {
        this.gitStatus = status;
        this._onDidChangeTreeData.fire(undefined);
      }
    } catch (error) {
      console.error('Failed to get local git status:', error);
    }
  }

  private async getLocalGitStatus(cwd: string): Promise<GitStatusResponse | null> {
    return new Promise((resolve) => {
      // Check if it's a git repo
      cp.exec('git rev-parse --is-inside-work-tree', { cwd }, (err) => {
        if (err) {
          resolve({ is_repo: false, branch: '', staged: [], modified: [], untracked: [] });
          return;
        }

        // Get branch name
        cp.exec('git branch --show-current', { cwd }, (branchErr, branchOut) => {
          const branch = branchErr ? 'HEAD' : branchOut.trim();

          // Get status
          cp.exec('git status --porcelain', { cwd }, (statusErr, statusOut) => {
            if (statusErr) {
              resolve({ is_repo: true, branch, staged: [], modified: [], untracked: [] });
              return;
            }

            const staged: string[] = [];
            const modified: string[] = [];
            const untracked: string[] = [];

            const lines = statusOut.trim().split('\n').filter(l => l.length > 0);
            for (const line of lines) {
              const index = line[0];
              const worktree = line[1];
              const file = line.slice(3);

              if (index === '?' && worktree === '?') {
                untracked.push(file);
              } else if (index !== ' ' && index !== '?') {
                staged.push(file);
              } else if (worktree !== ' ' && worktree !== '?') {
                modified.push(file);
              }
            }

            resolve({ is_repo: true, branch, staged, modified, untracked });
          });
        });
      });
    });
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

    if (element.type === 'status') {
      const item = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.None);
      item.description = element.description;
      item.iconPath = new vscode.ThemeIcon('git-branch');
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
      return [{ type: 'status', label: 'Not a git repository' }];
    }

    if (!element) {
      // Root level - return branch info and categories
      const items: GitItem[] = [];

      // Always show branch info at the top
      const branchInfo = this.getBranchInfo();
      if (branchInfo) {
        items.push({ type: 'status', label: branchInfo, description: 'current branch' });
      }

      if (this.gitStatus.staged.length > 0) {
        items.push({
          type: 'category',
          label: 'Staged Changes',
          files: this.gitStatus.staged.map(p => ({ path: p, type: 'staged' as const }))
        });
      }

      if (this.gitStatus.modified.length > 0) {
        items.push({
          type: 'category',
          label: 'Modified',
          files: this.gitStatus.modified.map(p => ({ path: p, type: 'modified' as const }))
        });
      }

      if (this.gitStatus.untracked.length > 0) {
        items.push({
          type: 'category',
          label: 'Untracked',
          files: this.gitStatus.untracked.map(p => ({ path: p, type: 'untracked' as const }))
        });
      }

      // Show clean status if no changes
      if (this.gitStatus.staged.length === 0 &&
          this.gitStatus.modified.length === 0 &&
          this.gitStatus.untracked.length === 0) {
        items.push({ type: 'status', label: 'Working tree clean', description: 'nothing to commit' });
      }

      return items;
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
