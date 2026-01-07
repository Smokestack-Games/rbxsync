import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';

export async function syncCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager
): Promise<void> {
  // Check connection
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('RbxSync: Not connected to server. Connect first.');
    return;
  }

  // Get project directory
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    vscode.window.showErrorMessage('RbxSync: No workspace folder open');
    return;
  }

  const projectDir = workspaceFolders[0].uri.fsPath;

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: 'RbxSync: Syncing to Studio...',
      cancellable: false
    },
    async (progress) => {
      statusBar.setSyncing();

      // Read the local tree
      progress.report({ message: 'Reading local files...' });
      const treeResult = await client.readTree(projectDir);

      if (!treeResult) {
        vscode.window.showErrorMessage('RbxSync: Failed to read local files');
        return;
      }

      // Build operations from instances
      const operations = treeResult.instances.map(inst => ({
        op: 'update' as const,
        path: inst.path,
        class_name: inst.class_name,
        name: inst.name,
        properties: inst.properties,
        source: inst.source
      }));

      if (operations.length === 0) {
        vscode.window.showInformationMessage('RbxSync: No changes to sync');
        return;
      }

      // Send batch sync
      progress.report({ message: `Syncing ${operations.length} instances...` });
      const syncResult = await client.syncBatch(operations);

      if (!syncResult) {
        vscode.window.showErrorMessage('RbxSync: Failed to sync to Studio');
        return;
      }

      if (syncResult.success) {
        const config = vscode.workspace.getConfiguration('rbxsync');
        if (config.get('showNotifications')) {
          vscode.window.showInformationMessage(
            `RbxSync: Synced ${syncResult.applied} instances to Studio`
          );
        }
      } else {
        const errorSummary = syncResult.errors.slice(0, 3).join('\n');
        vscode.window.showErrorMessage(
          `RbxSync: Sync completed with errors:\n${errorSummary}`
        );
      }
    }
  );

  // Refresh connection status
  await client.checkHealth();
}

export async function gitStatusCommand(
  client: RbxSyncClient
): Promise<void> {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    vscode.window.showErrorMessage('RbxSync: No workspace folder open');
    return;
  }

  const projectDir = workspaceFolders[0].uri.fsPath;
  const status = await client.getGitStatus(projectDir);

  if (!status) {
    vscode.window.showErrorMessage('RbxSync: Failed to get git status');
    return;
  }

  if (!status.is_repo) {
    vscode.window.showInformationMessage('RbxSync: Not a git repository');
    return;
  }

  const lines = [
    `Branch: ${status.branch || 'unknown'}`,
    `Staged: ${status.staged.length}`,
    `Modified: ${status.modified.length}`,
    `Untracked: ${status.untracked.length}`
  ];

  if (status.ahead) lines.push(`Ahead: ${status.ahead}`);
  if (status.behind) lines.push(`Behind: ${status.behind}`);

  vscode.window.showInformationMessage(lines.join(' | '));
}

export async function gitCommitCommand(
  client: RbxSyncClient
): Promise<void> {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    vscode.window.showErrorMessage('RbxSync: No workspace folder open');
    return;
  }

  const projectDir = workspaceFolders[0].uri.fsPath;

  // Get commit message
  const message = await vscode.window.showInputBox({
    prompt: 'Enter commit message',
    placeHolder: 'feat: add new feature'
  });

  if (!message) return;

  const result = await client.gitCommit(projectDir, message);

  if (!result) {
    vscode.window.showErrorMessage('RbxSync: Failed to commit');
    return;
  }

  if (result.success) {
    vscode.window.showInformationMessage(
      `RbxSync: Committed ${result.hash?.slice(0, 7) || 'successfully'}`
    );
  } else {
    vscode.window.showErrorMessage(`RbxSync: Commit failed - ${result.error}`);
  }
}
