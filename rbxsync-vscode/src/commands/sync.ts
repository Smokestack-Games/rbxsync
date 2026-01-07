import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';

export async function syncCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager,
  targetProjectDir?: string
): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('Not connected. Is Studio running?');
    return;
  }

  // Use provided projectDir or fall back to workspace
  let projectDir = targetProjectDir;
  if (!projectDir) {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders?.length) {
      vscode.window.showErrorMessage('Open a folder first.');
      return;
    }
    projectDir = workspaceFolders[0].uri.fsPath;
  }

  // Read and sync
  const treeResult = await client.readTree(projectDir);
  if (!treeResult) {
    vscode.window.showErrorMessage('Failed to read files.');
    return;
  }

  const operations = treeResult.instances.map(inst => ({
    type: 'update' as const,
    path: inst.path,
    data: inst
  }));

  if (operations.length === 0) {
    return; // Silent - nothing to sync
  }

  const syncResult = await client.syncBatch(operations, projectDir);

  if (!syncResult) {
    vscode.window.showErrorMessage('Sync failed. Try again.');
    return;
  }

  if (syncResult.success) {
    const config = vscode.workspace.getConfiguration('rbxsync');
    if (config.get('showNotifications')) {
      vscode.window.showInformationMessage(`Synced ${syncResult.applied} changes`);
    }
  } else if (syncResult.errors?.length) {
    vscode.window.showWarningMessage(`Synced with ${syncResult.errors.length} error(s)`);
  }
}
