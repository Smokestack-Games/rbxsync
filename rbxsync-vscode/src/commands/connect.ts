import * as vscode from 'vscode';
import * as http from 'http';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';

let serverTerminal: vscode.Terminal | null = null;

export async function connectCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager
): Promise<void> {
  // First check if server is already running
  let connected = await client.connect();

  if (connected) {
    // Register workspace with server
    if (client.projectDir) {
      await client.registerWorkspace(client.projectDir);
    }
    statusBar.startPolling();
    return;
  }

  // Server not running - start it
  vscode.window.showInformationMessage('Starting RbxSync server...');

  // Create or reuse terminal
  if (!serverTerminal || serverTerminal.exitStatus !== undefined) {
    serverTerminal = vscode.window.createTerminal({
      name: 'RbxSync Server',
      hideFromUser: false
    });
  }

  serverTerminal.sendText('rbxsync serve');
  serverTerminal.show(true);  // Show but don't take focus

  // Wait for server to start (poll up to 5 seconds)
  for (let i = 0; i < 10; i++) {
    await new Promise(resolve => setTimeout(resolve, 500));
    connected = await client.connect();
    if (connected) {
      // Register workspace with server
      if (client.projectDir) {
        await client.registerWorkspace(client.projectDir);
      }
      statusBar.startPolling();
      vscode.window.showInformationMessage('RbxSync server started');
      return;
    }
  }

  vscode.window.showErrorMessage('Failed to start server. Check the terminal for errors.');
}

export async function disconnectCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager
): Promise<void> {
  statusBar.stopPolling();

  // Send shutdown request to server
  try {
    const config = vscode.workspace.getConfiguration('rbxsync');
    const port = config.get<number>('serverPort') || 44755;

    await new Promise<void>((resolve, reject) => {
      const req = http.request({
        hostname: '127.0.0.1',
        port,
        path: '/shutdown',
        method: 'POST',
        timeout: 2000
      }, (res) => {
        resolve();
      });

      req.on('error', () => resolve());  // Server might close before responding
      req.on('timeout', () => {
        req.destroy();
        resolve();
      });

      req.end();
    });

    vscode.window.showInformationMessage('RbxSync server stopped');
  } catch {
    // Ignore errors - server might already be stopped
  }

  client['updateConnectionState']({ connected: false });
}
