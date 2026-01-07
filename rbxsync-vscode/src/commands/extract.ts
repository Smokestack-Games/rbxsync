import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';
import { InstanceTreeProvider } from '../views/treeView';

export async function extractCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager,
  treeProvider: InstanceTreeProvider
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

  // Ask which services to extract
  const serviceOptions = [
    { label: 'All Services', picked: true },
    { label: 'Workspace' },
    { label: 'ServerScriptService' },
    { label: 'ReplicatedStorage' },
    { label: 'ReplicatedFirst' },
    { label: 'StarterGui' },
    { label: 'StarterPack' },
    { label: 'StarterPlayer' },
    { label: 'ServerStorage' },
    { label: 'Lighting' },
    { label: 'SoundService' }
  ];

  const selected = await vscode.window.showQuickPick(serviceOptions, {
    canPickMany: true,
    placeHolder: 'Select services to extract (or leave empty for all)'
  });

  if (!selected) return; // Cancelled

  const services = selected.some(s => s.label === 'All Services')
    ? undefined
    : selected.map(s => s.label);

  // Start extraction
  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: 'RbxSync: Extracting game...',
      cancellable: true
    },
    async (progress, token) => {
      // Start extraction
      const startResult = await client.startExtraction(projectDir, services);
      if (!startResult) {
        vscode.window.showErrorMessage('RbxSync: Failed to start extraction');
        return;
      }

      const sessionId = startResult.session_id;

      // Poll for status
      let complete = false;
      while (!complete && !token.isCancellationRequested) {
        await new Promise(resolve => setTimeout(resolve, 500));

        const status = await client.getExtractionStatus();
        if (!status) continue;

        switch (status.status) {
          case 'extracting':
            const progressPercent = status.total_services > 0
              ? (status.services_complete / status.total_services) * 100
              : 0;
            progress.report({
              message: `${status.current_service || 'Processing'} (${status.instances_extracted} instances)`,
              increment: progressPercent
            });
            statusBar.setExtracting(
              status.current_service || 'Processing',
              status.services_complete / status.total_services
            );
            break;

          case 'complete':
            complete = true;
            break;

          case 'error':
            vscode.window.showErrorMessage(`RbxSync: Extraction error - ${status.error}`);
            return;
        }
      }

      if (token.isCancellationRequested) {
        vscode.window.showWarningMessage('RbxSync: Extraction cancelled');
        return;
      }

      // Finalize - write files
      progress.report({ message: 'Writing files...' });

      const finalizeResult = await client.finalizeExtraction(sessionId, projectDir);
      if (!finalizeResult || !finalizeResult.success) {
        vscode.window.showErrorMessage('RbxSync: Failed to write extracted files');
        return;
      }

      // Refresh tree view
      treeProvider.setProjectDir(projectDir);

      const config = vscode.workspace.getConfiguration('rbxsync');
      if (config.get('showNotifications')) {
        vscode.window.showInformationMessage(
          `RbxSync: Extracted ${finalizeResult.files_written} files`
        );
      }
    }
  );
}
