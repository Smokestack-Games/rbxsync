import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { ConnectionState } from '../server/types';

export class StatusBarManager {
  private statusBarItem: vscode.StatusBarItem;
  private pollingInterval: NodeJS.Timeout | null = null;

  constructor(private client: RbxSyncClient) {
    this.statusBarItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100
    );

    this.statusBarItem.command = 'rbxsync.connect';
    this.updateStatus(client.connectionState);

    // Listen for connection changes
    client.onConnectionChange((state) => {
      this.updateStatus(state);
    });
  }

  private updateStatus(state: ConnectionState): void {
    if (state.connected) {
      this.statusBarItem.text = '$(plug) RbxSync: Connected';
      this.statusBarItem.tooltip = `Connected to RbxSync server${state.serverVersion ? ` v${state.serverVersion}` : ''}`;
      this.statusBarItem.backgroundColor = undefined;
      this.statusBarItem.command = 'rbxsync.disconnect';
    } else {
      this.statusBarItem.text = '$(debug-disconnect) RbxSync: Disconnected';
      this.statusBarItem.tooltip = state.lastError || 'Click to connect';
      this.statusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
      this.statusBarItem.command = 'rbxsync.connect';
    }
  }

  show(): void {
    this.statusBarItem.show();
  }

  hide(): void {
    this.statusBarItem.hide();
  }

  startPolling(intervalMs: number = 5000): void {
    this.stopPolling();
    this.pollingInterval = setInterval(async () => {
      await this.client.checkHealth();
    }, intervalMs);
  }

  stopPolling(): void {
    if (this.pollingInterval) {
      clearInterval(this.pollingInterval);
      this.pollingInterval = null;
    }
  }

  setExtracting(current: string, progress: number): void {
    this.statusBarItem.text = `$(sync~spin) Extracting: ${current} (${Math.round(progress * 100)}%)`;
  }

  setSyncing(): void {
    this.statusBarItem.text = '$(sync~spin) Syncing to Studio...';
  }

  dispose(): void {
    this.stopPolling();
    this.statusBarItem.dispose();
  }
}
