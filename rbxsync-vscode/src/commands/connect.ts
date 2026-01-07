import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';

export async function connectCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager
): Promise<void> {
  const connected = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: 'RbxSync: Connecting to server...',
      cancellable: false
    },
    async () => {
      return await client.connect();
    }
  );

  if (connected) {
    const config = vscode.workspace.getConfiguration('rbxsync');
    if (config.get('showNotifications')) {
      vscode.window.showInformationMessage('RbxSync: Connected to Studio');
    }
    statusBar.startPolling();
  } else {
    const state = client.connectionState;
    const action = await vscode.window.showErrorMessage(
      `RbxSync: Connection failed - ${state.lastError || 'Unknown error'}`,
      'Retry',
      'Start Server'
    );

    if (action === 'Retry') {
      await connectCommand(client, statusBar);
    } else if (action === 'Start Server') {
      const terminal = vscode.window.createTerminal('RbxSync Server');
      terminal.sendText('rbxsync serve');
      terminal.show();

      // Wait a bit then retry
      await new Promise(resolve => setTimeout(resolve, 2000));
      await connectCommand(client, statusBar);
    }
  }
}

export async function disconnectCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager
): Promise<void> {
  statusBar.stopPolling();
  // Trigger a connection state update to show disconnected
  client['updateConnectionState']({ connected: false });

  const config = vscode.workspace.getConfiguration('rbxsync');
  if (config.get('showNotifications')) {
    vscode.window.showInformationMessage('RbxSync: Disconnected');
  }
}
