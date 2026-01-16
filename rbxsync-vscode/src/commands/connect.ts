import * as vscode from 'vscode';
import * as http from 'http';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';

let serverTerminal: vscode.Terminal | null = null;
let terminalCloseListener: vscode.Disposable | null = null;

/**
 * Initialize terminal tracking - call this during extension activation.
 * This prevents multiple terminal windows by clearing the reference when
 * the user closes the terminal manually (RBXSYNC-18).
 */
export function initServerTerminal(): vscode.Disposable {
  if (terminalCloseListener) {
    terminalCloseListener.dispose();
  }
  terminalCloseListener = vscode.window.onDidCloseTerminal((terminal) => {
    if (terminal === serverTerminal) {
      serverTerminal = null;
    }
  });
  return terminalCloseListener;
}

/**
 * Dispose server terminal and cleanup
 */
export function disposeServerTerminal(): void {
  if (serverTerminal) {
    serverTerminal.dispose();
    serverTerminal = null;
  }
  if (terminalCloseListener) {
    terminalCloseListener.dispose();
    terminalCloseListener = null;
  }
}

export async function connectCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager
): Promise<void> {
  let connected = await client.connect();

  if (connected) {
    if (client.projectDir) {
      await client.registerWorkspace(client.projectDir);
    }
    statusBar.startPolling();
    return;
  }

  vscode.window.showInformationMessage('Starting RbxSync server...');

  if (!serverTerminal || serverTerminal.exitStatus !== undefined) {
    serverTerminal = vscode.window.createTerminal({
      name: 'RbxSync Server',
      hideFromUser: false
    });
  }

  serverTerminal.sendText('rbxsync serve');
  serverTerminal.show(true);

  for (let i = 0; i < 10; i++) {
    await new Promise(resolve => setTimeout(resolve, 500));
    connected = await client.connect();
    if (connected) {
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

      req.on('error', () => resolve());
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
