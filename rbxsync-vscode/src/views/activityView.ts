import * as vscode from 'vscode';
import { PlaceInfo } from '../server/types';

type ItemType = 'header' | 'place' | 'place-detail' | 'action' | 'result' | 'separator';

interface StatusItem {
  id: string;
  type: ItemType;
  label: string;
  description?: string;
  icon?: string;
  iconColor?: vscode.ThemeColor;
  command?: vscode.Command;
  contextValue?: string;
  isLinked?: boolean;  // This place matches current workspace
}

export class ActivityViewProvider implements vscode.TreeDataProvider<StatusItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<StatusItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private connectionStatus: 'connected' | 'disconnected' | 'connecting' = 'disconnected';
  private allPlaces: PlaceInfo[] = [];
  private currentProjectDir: string = '';
  private currentOperation: string | null = null;
  private lastResult: { label: string; success: boolean; time: Date } | null = null;
  private clearResultTimeout: NodeJS.Timeout | null = null;

  setConnectionStatus(
    status: 'connected' | 'disconnected' | 'connecting',
    places: PlaceInfo[] = [],
    currentProjectDir: string = ''
  ): void {
    this.connectionStatus = status;
    this.allPlaces = places;
    this.currentProjectDir = currentProjectDir;
    this._onDidChangeTreeData.fire(undefined);
  }

  updatePlaces(places: PlaceInfo[], currentProjectDir: string): void {
    this.allPlaces = places;
    this.currentProjectDir = currentProjectDir;
    this._onDidChangeTreeData.fire(undefined);
  }

  setCurrentOperation(operation: string | null): void {
    this.currentOperation = operation;
    this._onDidChangeTreeData.fire(undefined);
  }

  logSync(count: number): void {
    this.setResult(`Synced ${count} change${count !== 1 ? 's' : ''}`, true);
  }

  logExtract(count: number): void {
    this.setResult(`Extracted ${count} file${count !== 1 ? 's' : ''}`, true);
  }

  logTest(duration: number, messages: number): void {
    this.setResult(`Test complete (${messages} messages)`, true);
  }

  logError(message: string): void {
    this.setResult(message, false);
  }

  private setResult(label: string, success: boolean): void {
    this.lastResult = { label, success, time: new Date() };
    this._onDidChangeTreeData.fire(undefined);

    if (this.clearResultTimeout) {
      clearTimeout(this.clearResultTimeout);
    }
    this.clearResultTimeout = setTimeout(() => {
      this.lastResult = null;
      this._onDidChangeTreeData.fire(undefined);
    }, 30000);
  }

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  getTreeItem(element: StatusItem): vscode.TreeItem {
    const item = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.None);

    if (element.description) {
      item.description = element.description;
    }

    if (element.icon) {
      item.iconPath = new vscode.ThemeIcon(element.icon, element.iconColor);
    }

    if (element.command) {
      item.command = element.command;
    }

    if (element.contextValue) {
      item.contextValue = element.contextValue;
    }

    // Style headers differently
    if (element.type === 'header') {
      item.collapsibleState = vscode.TreeItemCollapsibleState.None;
    }

    // Style place details with indentation feel
    if (element.type === 'place-detail') {
      item.description = element.description;
    }

    return item;
  }

  getChildren(): StatusItem[] {
    const items: StatusItem[] = [];
    const connected = this.connectionStatus === 'connected';
    const connecting = this.connectionStatus === 'connecting';

    // ═══════════════════════════════════════════════════════════
    // STUDIOS ONLINE SECTION
    // ═══════════════════════════════════════════════════════════
    const studioCount = this.allPlaces.length;
    const linkedPlace = this.allPlaces.find(p => p.project_dir === this.currentProjectDir);

    items.push({
      id: 'header-studios',
      type: 'header',
      label: 'STUDIOS ONLINE',
      description: connected ? String(studioCount) : connecting ? '...' : '0',
      icon: connected ? 'broadcast' : connecting ? 'loading~spin' : 'circle-outline',
      iconColor: connected && studioCount > 0
        ? new vscode.ThemeColor('testing.iconPassed')
        : connecting
          ? new vscode.ThemeColor('charts.blue')
          : new vscode.ThemeColor('disabledForeground')
    });

    // Show each connected place
    if (connected && studioCount > 0) {
      // Sort: linked place first, then alphabetically
      const sortedPlaces = [...this.allPlaces].sort((a, b) => {
        const aLinked = a.project_dir === this.currentProjectDir;
        const bLinked = b.project_dir === this.currentProjectDir;
        if (aLinked && !bLinked) return -1;
        if (!aLinked && bLinked) return 1;
        return a.place_name.localeCompare(b.place_name);
      });

      for (const place of sortedPlaces) {
        const isLinked = place.project_dir === this.currentProjectDir;

        // Place name row
        items.push({
          id: `place-${place.place_id}`,
          type: 'place',
          label: place.place_name,
          description: isLinked ? '← LINKED' : '',
          icon: isLinked ? 'circle-filled' : 'circle-outline',
          iconColor: isLinked
            ? new vscode.ThemeColor('testing.iconPassed')
            : new vscode.ThemeColor('foreground'),
          isLinked,
          contextValue: 'place'
        });

        // Place details (ID + path)
        const shortPath = this.shortenPath(place.project_dir);
        items.push({
          id: `place-detail-${place.place_id}`,
          type: 'place-detail',
          label: `    ${place.place_id}`,
          description: shortPath,
          icon: 'blank',  // Indent
          iconColor: new vscode.ThemeColor('disabledForeground')
        });

        // Quick actions for this place
        items.push({
          id: `place-actions-${place.place_id}`,
          type: 'place-detail',
          label: '    ↑ Sync',
          description: '↓ Extract',
          icon: 'blank',
          command: isLinked ? { command: 'rbxsync.sync', title: 'Sync' } : undefined
        });
      }
    } else if (connected) {
      // Server connected but no Studio places
      items.push({
        id: 'no-studios',
        type: 'place-detail',
        label: '    Waiting for Studio...',
        icon: 'blank',
        iconColor: new vscode.ThemeColor('disabledForeground')
      });
    } else if (!connecting) {
      // Disconnected
      items.push({
        id: 'disconnected-hint',
        type: 'place-detail',
        label: '    Start server to connect',
        icon: 'blank',
        iconColor: new vscode.ThemeColor('disabledForeground')
      });
    }

    // ═══════════════════════════════════════════════════════════
    // CURRENT OPERATION
    // ═══════════════════════════════════════════════════════════
    if (this.currentOperation) {
      items.push({
        id: 'header-working',
        type: 'header',
        label: 'WORKING',
        description: this.currentOperation,
        icon: 'sync~spin',
        iconColor: new vscode.ThemeColor('charts.blue')
      });
    }

    // ═══════════════════════════════════════════════════════════
    // ACTIONS SECTION
    // ═══════════════════════════════════════════════════════════
    items.push({
      id: 'header-actions',
      type: 'header',
      label: 'ACTIONS',
      icon: 'zap'
    });

    // Connect/Disconnect
    items.push({
      id: 'action-connect',
      type: 'action',
      label: connected ? '    Disconnect' : connecting ? '    Connecting...' : '    Connect',
      icon: connected ? 'debug-disconnect' : connecting ? 'loading~spin' : 'plug',
      iconColor: connecting ? new vscode.ThemeColor('charts.blue') : undefined,
      command: connecting ? undefined : {
        command: connected ? 'rbxsync.disconnect' : 'rbxsync.connect',
        title: connected ? 'Disconnect' : 'Connect'
      }
    });

    if (connected && linkedPlace) {
      items.push({
        id: 'action-sync',
        type: 'action',
        label: '    Sync to Studio',
        icon: 'cloud-upload',
        command: { command: 'rbxsync.sync', title: 'Sync' }
      });

      items.push({
        id: 'action-extract',
        type: 'action',
        label: '    Extract from Studio',
        icon: 'cloud-download',
        command: { command: 'rbxsync.extract', title: 'Extract' }
      });

      items.push({
        id: 'action-test',
        type: 'action',
        label: '    Run Play Test',
        icon: 'play',
        command: { command: 'rbxsync.runTest', title: 'Play Test' }
      });
    }

    items.push({
      id: 'action-refresh',
      type: 'action',
      label: '    Refresh',
      icon: 'refresh',
      command: { command: 'rbxsync.refresh', title: 'Refresh' }
    });

    // ═══════════════════════════════════════════════════════════
    // LAST ACTION SECTION
    // ═══════════════════════════════════════════════════════════
    if (this.lastResult) {
      const ago = this.getRelativeTime(this.lastResult.time);
      items.push({
        id: 'header-last',
        type: 'header',
        label: 'LAST ACTION',
        icon: this.lastResult.success ? 'check' : 'error',
        iconColor: this.lastResult.success
          ? new vscode.ThemeColor('testing.iconPassed')
          : new vscode.ThemeColor('testing.iconFailed')
      });

      items.push({
        id: 'last-result',
        type: 'result',
        label: `    ${this.lastResult.label}`,
        description: ago,
        icon: 'blank'
      });
    }

    return items;
  }

  private shortenPath(fullPath: string): string {
    // Convert to relative-like display
    const home = process.env.HOME || process.env.USERPROFILE || '';
    if (fullPath.startsWith(home)) {
      return '~' + fullPath.slice(home.length);
    }
    // Just show last 2 segments
    const parts = fullPath.split('/').filter(Boolean);
    if (parts.length > 2) {
      return '.../' + parts.slice(-2).join('/');
    }
    return fullPath;
  }

  private getRelativeTime(date: Date): string {
    const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
    if (seconds < 5) return 'just now';
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    return date.toLocaleTimeString();
  }

  dispose(): void {
    if (this.clearResultTimeout) {
      clearTimeout(this.clearResultTimeout);
    }
    this._onDidChangeTreeData.dispose();
  }
}
