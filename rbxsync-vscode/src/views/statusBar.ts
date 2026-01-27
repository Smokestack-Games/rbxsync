import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { ConnectionState, PlaceInfo, OperationInfo } from '../server/types';

export class StatusBarManager {
  private statusBarItem: vscode.StatusBarItem;
  private pollingInterval: NodeJS.Timeout | null = null;
  private isBusy = false;
  private allPlaces: PlaceInfo[] = [];
  private currentProjectDir: string = '';
  private onPlacesChangeCallback?: (places: PlaceInfo[], projectDir: string) => void;
  private onOperationStatusChangeCallback?: (operation: OperationInfo | null) => void;
  private lastOperationStatus: OperationInfo | null = null;

  constructor(private client: RbxSyncClient) {
    this.statusBarItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100
    );

    this.statusBarItem.command = 'rbxsync.connect';
    this.updateStatus(client.connectionState);

    // Listen for connection changes
    client.onConnectionChange((state) => {
      if (!this.isBusy) {
        this.updateStatus(state);
      }
    });
  }

  updatePlaces(places: PlaceInfo[], currentProjectDir: string): void {
    this.allPlaces = places;
    this.currentProjectDir = currentProjectDir;
    if (!this.isBusy) {
      this.updateStatus(this.client.connectionState);
    }
    // Notify callback of places change
    this.onPlacesChangeCallback?.(places, currentProjectDir);
  }

  onPlacesChange(callback: (places: PlaceInfo[], projectDir: string) => void): void {
    this.onPlacesChangeCallback = callback;
  }

  // Callback for operation status changes (RBXSYNC-77)
  onOperationStatusChange(callback: (operation: OperationInfo | null) => void): void {
    this.onOperationStatusChangeCallback = callback;
  }

  private updateStatus(state: ConnectionState): void {
    if (state.connected) {
      const studioCount = this.allPlaces.length;
      const linkedPlace = this.allPlaces.find(p => p.project_dir === this.currentProjectDir);

      if (studioCount > 0) {
        // Show count and linked place name
        // Format: "● 2 Studios │ My Game ←→"
        if (linkedPlace) {
          if (studioCount > 1) {
            this.statusBarItem.text = `$(circle-filled) ${studioCount} Studios │ ${linkedPlace.place_name} ←→`;
          } else {
            this.statusBarItem.text = `$(circle-filled) ${linkedPlace.place_name} ←→`;
          }
          this.statusBarItem.tooltip = this.buildTooltip(linkedPlace, studioCount);
        } else {
          // Studios online but none linked to this workspace
          this.statusBarItem.text = `$(circle-filled) ${studioCount} Studio${studioCount > 1 ? 's' : ''} │ Not linked`;
          this.statusBarItem.tooltip = `${studioCount} Studio instance${studioCount > 1 ? 's' : ''} connected\nNone linked to this workspace\n\nClick to view details`;
        }
        this.statusBarItem.color = new vscode.ThemeColor('testing.iconPassed');
      } else {
        // Server connected but no Studios
        this.statusBarItem.text = '$(circle-filled) RbxSync │ Waiting...';
        this.statusBarItem.tooltip = 'Server connected\nWaiting for Studio to connect\n\nClick to view details';
        this.statusBarItem.color = new vscode.ThemeColor('charts.yellow');
      }
      this.statusBarItem.backgroundColor = undefined;
      this.statusBarItem.command = 'rbxsync.sidebarView.focus';
    } else {
      this.statusBarItem.text = '$(circle-outline) RbxSync';
      this.statusBarItem.tooltip = state.lastError
        ? `${state.lastError}\nClick to connect`
        : 'Disconnected\nClick to connect';
      this.statusBarItem.color = undefined;
      this.statusBarItem.backgroundColor = undefined;
      this.statusBarItem.command = 'rbxsync.connect';
    }
  }

  private buildTooltip(linkedPlace: PlaceInfo, totalCount: number): string {
    const lines = [
      `⬤ ${linkedPlace.place_name}`,
      `  PlaceId: ${linkedPlace.place_id}`,
      `  Project: ${this.shortenPath(linkedPlace.project_dir)}`,
      '',
    ];

    if (totalCount > 1) {
      lines.push(`${totalCount - 1} other Studio${totalCount > 2 ? 's' : ''} connected`);
      lines.push('');
    }

    lines.push('Click to view all connections');
    return lines.join('\n');
  }

  private shortenPath(fullPath: string): string {
    const home = process.env.HOME || process.env.USERPROFILE || '';
    if (fullPath.startsWith(home)) {
      return '~' + fullPath.slice(home.length);
    }
    return fullPath;
  }

  show(): void {
    this.statusBarItem.show();
  }

  hide(): void {
    this.statusBarItem.hide();
  }

  startPolling(intervalMs: number = 5000): void {
    this.stopPolling();

    // Poll for operation status more frequently (500ms) when connected (RBXSYNC-77)
    const operationPollInterval = setInterval(async () => {
      if (this.client.connectionState.connected && this.currentProjectDir) {
        const operation = await this.client.getOperationStatus(this.currentProjectDir);

        // Check if operation status changed
        const changed = (operation === null && this.lastOperationStatus !== null) ||
                       (operation !== null && this.lastOperationStatus === null) ||
                       (operation?.type !== this.lastOperationStatus?.type) ||
                       (operation?.startTime !== this.lastOperationStatus?.startTime);

        if (changed) {
          this.lastOperationStatus = operation;
          this.onOperationStatusChangeCallback?.(operation);

          // Update busy status based on operation
          if (operation) {
            const opName = operation.type === 'extract' ? 'Extracting' :
                          operation.type === 'sync' ? 'Syncing' : 'Testing';
            this.setBusy(operation.progress || opName);
          } else {
            this.clearBusy();
          }
        }
      }
    }, 500);

    // Store operation poll interval for cleanup
    (this as unknown as { operationPollInterval: NodeJS.Timeout }).operationPollInterval = operationPollInterval;

    this.pollingInterval = setInterval(async () => {
      await this.client.checkHealth();
      // Re-register workspace as heartbeat
      if (this.currentProjectDir) {
        await this.client.registerWorkspace(this.currentProjectDir);
      }
      // Refresh places list
      if (this.client.connectionState.connected) {
        const places = await this.client.getConnectedPlaces();
        this.updatePlaces(places, this.currentProjectDir);
      }
    }, intervalMs);
  }

  stopPolling(): void {
    if (this.pollingInterval) {
      clearInterval(this.pollingInterval);
      this.pollingInterval = null;
    }
    // Clear operation poll interval (RBXSYNC-77)
    const operationInterval = (this as unknown as { operationPollInterval?: NodeJS.Timeout }).operationPollInterval;
    if (operationInterval) {
      clearInterval(operationInterval);
      (this as unknown as { operationPollInterval?: NodeJS.Timeout }).operationPollInterval = undefined;
    }
  }

  setBusy(message?: string): void {
    this.isBusy = true;
    this.statusBarItem.text = '$(sync~spin) RbxSync';
    this.statusBarItem.tooltip = message || 'Working...';
    this.statusBarItem.color = new vscode.ThemeColor('charts.blue');
  }

  clearBusy(): void {
    this.isBusy = false;
    this.updateStatus(this.client.connectionState);
  }

  // Legacy methods for compatibility - redirect to new API
  setExtracting(current: string, _progress: number): void {
    this.setBusy(`Extracting ${current}`);
  }

  setSyncing(): void {
    this.setBusy('Syncing');
  }

  dispose(): void {
    this.stopPolling();
    this.statusBarItem.dispose();
  }
}
