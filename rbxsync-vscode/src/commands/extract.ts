import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';
import { SidebarWebviewProvider } from '../views/sidebarWebview';
import { generateProjectJson, addToGitignore } from '../lsp/projectJson';

export async function extractCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager,
  sidebarView: SidebarWebviewProvider,
  targetProjectDir?: string,
  placeId?: number,
  sessionId?: string | null,
  includeTerrain: boolean = true
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

  // Run extraction (extract all services)
  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: 'Extracting...', cancellable: true },
    async (progress, token) => {
      const startResult = await client.startExtraction(projectDir, undefined, includeTerrain);
      if (!startResult) {
        sidebarView.logError('Extraction failed to start', placeId, sessionId);
        return;
      }

      const extractSessionId = startResult.session_id;
      let complete = false;

      while (!complete && !token.isCancellationRequested) {
        await new Promise(r => setTimeout(r, 500));
        const status = await client.getExtractionStatus();
        if (!status) continue;

        if (status.error) {
          sidebarView.logError(status.error || 'Extraction failed', placeId, sessionId);
          return;
        }

        if (status.complete) {
          complete = true;
        } else {
          progress.report({ message: `${status.chunksReceived}/${status.totalChunks} chunks` });
        }
      }

      if (token.isCancellationRequested) return;

      progress.report({ message: 'Writing files...' });
      const result = await client.finalizeExtraction(extractSessionId, projectDir);

      if (!result?.success) {
        sidebarView.logError('Failed to write files', placeId, sessionId);
        return;
      }

      const totalFiles = (result.filesWritten || 0) + (result.scriptsWritten || 0);
      sidebarView.logExtract(totalFiles, placeId, sessionId);

      // Generate default.project.json for Luau LSP compatibility (RBXSYNC-19)
      const config = vscode.workspace.getConfiguration('rbxsync');
      if (config.get('generateProjectJson', true)) {
        try {
          const projectJsonPath = await generateProjectJson(projectDir);
          if (projectJsonPath) {
            await addToGitignore(projectDir);
          }
        } catch (e) {
          console.error('Failed to generate project.json:', e);
        }
      }

      if (config.get('showNotifications')) {
        vscode.window.showInformationMessage(`Extracted ${totalFiles} files (${result.scriptsWritten || 0} scripts)`);
      }
    }
  );
}
