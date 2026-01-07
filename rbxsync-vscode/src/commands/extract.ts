import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';
import { ActivityViewProvider } from '../views/activityView';

export async function extractCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager,
  activityView: ActivityViewProvider,
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

  // Quick service picker
  const services = [
    { label: 'All Services', picked: true },
    { label: 'Workspace' },
    { label: 'ServerScriptService' },
    { label: 'ReplicatedStorage' },
    { label: 'StarterGui' },
    { label: 'StarterPlayer' },
    { label: 'ServerStorage' }
  ];

  const selected = await vscode.window.showQuickPick(services, {
    canPickMany: true,
    placeHolder: 'Services to extract'
  });

  if (!selected) return;

  const serviceList = selected.some(s => s.label === 'All Services')
    ? undefined
    : selected.map(s => s.label);

  // Run extraction
  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: 'Extracting...', cancellable: true },
    async (progress, token) => {
      const startResult = await client.startExtraction(projectDir, serviceList);
      if (!startResult) {
        activityView.logError('Extraction failed to start');
        return;
      }

      const sessionId = startResult.session_id;
      let complete = false;

      while (!complete && !token.isCancellationRequested) {
        await new Promise(r => setTimeout(r, 500));
        const status = await client.getExtractionStatus();
        if (!status) continue;

        if (status.status === 'extracting') {
          progress.report({ message: `${status.instances_extracted} instances` });
        } else if (status.status === 'complete') {
          complete = true;
        } else if (status.status === 'error') {
          activityView.logError(status.error || 'Extraction failed');
          return;
        }
      }

      if (token.isCancellationRequested) return;

      progress.report({ message: 'Writing files...' });
      const result = await client.finalizeExtraction(sessionId, projectDir);

      if (!result?.success) {
        activityView.logError('Failed to write files');
        return;
      }

      activityView.logExtract(result.files_written || 0);

      const config = vscode.workspace.getConfiguration('rbxsync');
      if (config.get('showNotifications')) {
        vscode.window.showInformationMessage(`Extracted ${result.files_written} files`);
      }
    }
  );
}
