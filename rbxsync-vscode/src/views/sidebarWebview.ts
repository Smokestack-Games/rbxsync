import * as vscode from 'vscode';
import { PlaceInfo } from '../server/types';

interface StudioOperation {
  type: 'sync' | 'extract' | 'test';
  status: 'running' | 'success' | 'error';
  message: string;
  startTime: number;
  endTime?: number;
}

interface SidebarState {
  connectionStatus: 'connected' | 'disconnected' | 'connecting';
  places: PlaceInfo[];
  currentProjectDir: string;
  currentOperation: string | null;
  lastResult: { label: string; success: boolean; time: number } | null;
  e2eModeEnabled: boolean;
  serverRunning: boolean;
  // Keyed by studioKey (place_id or fallback name-based key)
  studioOperations: { [studioKey: string]: StudioOperation };
  // Enhanced settings
  serverPort: number;
  testDuration: number;
  extractTerrain: boolean;
  updateAvailable: string | null;
  // Zen cat mascot state
  catMood: 'idle' | 'syncing' | 'success' | 'error';
  catOperationType: 'sync' | 'extract' | 'test' | null;
  // Rbxjson files visibility
  rbxjsonHidden: boolean;
  // Cat panel visibility
  catVisible: boolean;
}

/**
 * Generate a unique key for a studio place.
 * Uses session_id if available (most reliable), then place_id if > 0, otherwise falls back to place_name.
 */
function getStudioKey(place: PlaceInfo | { place_id?: number; place_name?: string; session_id?: string }, index?: number): string {
  // Prefer session_id as it's unique per Studio instance
  if ('session_id' in place && place.session_id) {
    return `session_${place.session_id}`;
  }
  // For published places, place_id is unique
  if (place.place_id && place.place_id > 0) {
    return `id_${place.place_id}`;
  }
  // Fallback: use place_name with optional index for uniqueness
  const name = place.place_name || 'unknown';
  return index !== undefined ? `name_${name}_${index}` : `name_${name}`;
}

export class SidebarWebviewProvider implements vscode.WebviewViewProvider {
  public static readonly viewType = 'rbxsync.sidebarView';

  private _view?: vscode.WebviewView;
  private _extensionUri: vscode.Uri;
  private _version: string;

  private state: SidebarState = {
    connectionStatus: 'disconnected',
    places: [],
    currentProjectDir: '',
    currentOperation: null,
    lastResult: null,
    e2eModeEnabled: false,
    serverRunning: false,
    studioOperations: {},
    serverPort: 44755,
    testDuration: 5,
    extractTerrain: true,
    updateAvailable: null,
    catMood: 'idle',
    catOperationType: null,
    rbxjsonHidden: true,
    catVisible: true
  };

  constructor(extensionUri: vscode.Uri, version?: string) {
    this._extensionUri = extensionUri;
    this._version = version || '1.1.0';
  }

  public resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken
  ): void {
    this._view = webviewView;

    // Set the title to show version with dash separator
    webviewView.title = `- v${this._version}`;

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [this._extensionUri]
    };

    webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

    // Handle messages from the webview
    webviewView.webview.onDidReceiveMessage(async (message) => {
      switch (message.command) {
        case 'connect':
          vscode.commands.executeCommand('rbxsync.connect');
          break;
        case 'disconnect':
          vscode.commands.executeCommand('rbxsync.disconnect');
          break;
        case 'sync':
          // Pass projectDir, placeId, and sessionId for proper routing and operation tracking
          vscode.commands.executeCommand('rbxsync.syncTo', message.projectDir || this.state.currentProjectDir, message.placeId, message.sessionId);
          break;
        case 'extract':
          vscode.commands.executeCommand('rbxsync.extractFrom', message.projectDir || this.state.currentProjectDir, message.placeId, message.sessionId);
          break;
        case 'test':
          vscode.commands.executeCommand('rbxsync.runTestOn', message.projectDir || this.state.currentProjectDir, message.placeId, message.sessionId);
          break;
        case 'openConsole':
          vscode.commands.executeCommand('rbxsync.openConsole');
          break;
        case 'toggleE2E':
          vscode.commands.executeCommand('rbxsync.toggleE2EMode');
          break;
        case 'toggleRbxjson':
          vscode.commands.executeCommand('rbxsync.toggleMetadataFiles');
          break;
        case 'refresh':
          vscode.commands.executeCommand('rbxsync.refresh');
          break;
        case 'linkStudio':
          vscode.commands.executeCommand('rbxsync.linkStudio', message.placeId);
          break;
        case 'unlinkStudio':
          vscode.commands.executeCommand('rbxsync.unlinkStudio', message.placeId);
          break;
        case 'ready':
          this._updateWebview();
          break;
        case 'setTestDuration':
          this.state.testDuration = message.value;
          break;
        case 'setExtractTerrain':
          this.state.extractTerrain = message.value;
          break;
        case 'dismissUpdate':
          this.state.updateAvailable = null;
          this._updateWebview();
          break;
      }
    });
  }

  public setConnectionStatus(
    status: 'connected' | 'disconnected' | 'connecting',
    places: PlaceInfo[] = [],
    currentProjectDir: string = ''
  ): void {
    this.state.connectionStatus = status;
    this.state.places = places;
    this.state.currentProjectDir = currentProjectDir;
    this.state.serverRunning = status === 'connected';
    this._updateWebview();
  }

  public updatePlaces(places: PlaceInfo[], currentProjectDir: string): void {
    this.state.places = places;
    this.state.currentProjectDir = currentProjectDir;

    // Clear any stale "running" operations that are older than 2 minutes
    const now = Date.now();
    for (const studioKey in this.state.studioOperations) {
      const op = this.state.studioOperations[studioKey];
      if (op.status === 'running' && (now - op.startTime) > 120000) {
        delete this.state.studioOperations[studioKey];
      }
    }

    this._updateWebview();
  }

  public setCurrentOperation(operation: string | null): void {
    this.state.currentOperation = operation;
    this._updateWebview();
  }

  public setE2EMode(enabled: boolean): void {
    this.state.e2eModeEnabled = enabled;
    this._updateWebview();
  }

  public setRbxjsonHidden(hidden: boolean): void {
    this.state.rbxjsonHidden = hidden;
    this._updateWebview();
  }

  public toggleCat(): boolean {
    this.state.catVisible = !this.state.catVisible;
    this._updateWebview();
    return this.state.catVisible;
  }

  public setUpdateAvailable(version: string | null): void {
    this.state.updateAvailable = version;
    this._updateWebview();
  }

  public setServerPort(port: number): void {
    this.state.serverPort = port;
    this._updateWebview();
  }

  // Update zen cat mood based on current operations
  public setCatMood(mood: 'idle' | 'syncing' | 'success' | 'error'): void {
    this.state.catMood = mood;
    this._updateWebview();
  }

  // Studio operation tracking - keyed by sessionId or placeId
  public startStudioOperation(placeId: number, type: 'sync' | 'extract' | 'test', sessionId?: string | null): void {
    const studioKey = getStudioKey({ place_id: placeId, session_id: sessionId || undefined });
    this.state.studioOperations[studioKey] = {
      type,
      status: 'running',
      message: type === 'sync' ? 'Syncing...' : type === 'extract' ? 'Extracting...' : 'Testing...',
      startTime: Date.now()
    };
    this.state.catMood = 'syncing';
    this.state.catOperationType = type;
    this._updateWebview();
  }

  public completeStudioOperation(placeId: number, success: boolean, message: string, sessionId?: string | null): void {
    const studioKey = getStudioKey({ place_id: placeId, session_id: sessionId || undefined });
    const op = this.state.studioOperations[studioKey];
    if (op) {
      op.status = success ? 'success' : 'error';
      op.message = message;
      op.endTime = Date.now();
      this.state.catMood = success ? 'success' : 'error';
      this._updateWebview();

      // Reset cat mood and clear operation after delay
      setTimeout(() => {
        if (this.state.studioOperations[studioKey] === op) {
          delete this.state.studioOperations[studioKey];
          this.state.catMood = 'idle';
          this.state.catOperationType = null;
          this._updateWebview();
        }
      }, 30000);
    }
  }

  public logSync(count: number, placeId?: number, sessionId?: string | null): void {
    const message = `Synced ${count} change${count !== 1 ? 's' : ''}`;
    if (placeId !== undefined) {
      this.completeStudioOperation(placeId, true, message, sessionId);
    }
    this._setResult(message, true);
  }

  public logExtract(count: number, placeId?: number, sessionId?: string | null): void {
    const message = `Extracted ${count} file${count !== 1 ? 's' : ''}`;
    if (placeId !== undefined) {
      this.completeStudioOperation(placeId, true, message, sessionId);
    }
    this._setResult(message, true);
  }

  public logTest(duration: number, messages: number, placeId?: number, sessionId?: string | null): void {
    const message = `Test complete (${messages} messages)`;
    if (placeId !== undefined) {
      this.completeStudioOperation(placeId, true, message, sessionId);
    }
    this._setResult(message, true);
  }

  public logError(message: string, placeId?: number, sessionId?: string | null): void {
    if (placeId !== undefined) {
      this.completeStudioOperation(placeId, false, message, sessionId);
    }
    this._setResult(message, false);
  }

  // Handle server-initiated operation status updates (RBXSYNC-77)
  // This is called when CLI/MCP starts/stops operations
  public handleServerOperation(operation: { type: 'extract' | 'sync' | 'test'; project_dir: string; progress?: string } | null): void {
    if (operation) {
      // Find the linked place for this project_dir
      const linkedPlace = this.state.places.find(p => p.project_dir === operation.project_dir);
      if (linkedPlace) {
        const placeId = linkedPlace.place_id;
        const sessionId = linkedPlace.session_id;

        // Check if we already have a running operation for this place
        const studioKey = getStudioKey({ place_id: placeId, session_id: sessionId });
        const existingOp = this.state.studioOperations[studioKey];
        if (!existingOp || existingOp.status !== 'running') {
          this.startStudioOperation(placeId, operation.type, sessionId);
        }
      }

      // Update cat mood
      this.state.catMood = 'syncing';
      this.state.catOperationType = operation.type;
      this.state.currentOperation = operation.progress || (operation.type === 'extract' ? 'Extracting...' :
                                    operation.type === 'sync' ? 'Syncing...' : 'Testing...');
      this._updateWebview();
    } else {
      // Operation completed - find any running operations and complete them
      for (const studioKey in this.state.studioOperations) {
        const op = this.state.studioOperations[studioKey];
        if (op.status === 'running') {
          op.status = 'success';
          op.endTime = Date.now();
          op.message = op.type === 'extract' ? 'Extraction complete' :
                       op.type === 'sync' ? 'Sync complete' : 'Test complete';
        }
      }
      this.state.catMood = 'success';
      this.state.currentOperation = null;
      this._updateWebview();

      // Reset to idle after delay
      setTimeout(() => {
        this.state.catMood = 'idle';
        this.state.catOperationType = null;
        // Clear completed operations
        for (const studioKey in this.state.studioOperations) {
          const op = this.state.studioOperations[studioKey];
          if (op.status !== 'running') {
            delete this.state.studioOperations[studioKey];
          }
        }
        this._updateWebview();
      }, 3000);
    }
  }

  private _setResult(label: string, success: boolean): void {
    this.state.lastResult = { label, success, time: Date.now() };
    this._updateWebview();

    setTimeout(() => {
      if (this.state.lastResult && this.state.lastResult.time === this.state.lastResult.time) {
        this.state.lastResult = null;
        this._updateWebview();
      }
    }, 30000);
  }

  public refresh(): void {
    this._updateWebview();
  }

  private _updateWebview(): void {
    if (this._view) {
      this._view.webview.postMessage({ type: 'stateUpdate', state: this.state });
    }
  }

  public dispose(): void {}

  private _getHtmlForWebview(webview: vscode.Webview): string {
    const nonce = getNonce();

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}'; img-src ${webview.cspSource} data:;">
  <style>
    :root {
      /* Unified RbxSync Design System */
      --accent: #4ADE80;
      --accent-hover: #5EEA94;
      --accent-muted: #22543A;
      --accent-soft: rgba(74, 222, 128, 0.15);

      --success: #4ADE80;
      --success-soft: rgba(74, 222, 128, 0.15);
      --warning: #FACC15;
      --warning-soft: rgba(250, 204, 21, 0.15);
      --error: #F87171;
      --error-soft: rgba(248, 113, 113, 0.15);
      --blue: #60A5FA;
      --blue-soft: rgba(96, 165, 250, 0.15);
      --purple: #8b5cf6;
      --purple-soft: rgba(139, 92, 246, 0.15);

      /* Core backgrounds - unified with Studio plugin */
      --bg-base: #18181B;
      --bg-surface: #202024;
      --bg-elevated: #2D2D32;
      --bg-hover: #2D2D32;
      --bg-active: #373740;

      /* Text hierarchy - unified */
      --text-primary: #F4F4F5;
      --text-secondary: #A1A1AA;
      --text-muted: #71717A;

      /* Borders - unified */
      --border: #2D2D32;
      --border-light: #3C3C44;
      --border-focus: #3C3C44;

      --radius: 8px;
      --radius-sm: 6px;
      --radius-xs: 4px;
    }

    * { margin: 0; padding: 0; box-sizing: border-box; }

    body {
      font-family: var(--vscode-font-family, system-ui, sans-serif);
      font-size: 12px;
      color: var(--text-primary);
      background: var(--bg-base);
      padding: 4px 12px 12px 12px;
      line-height: 1.5;
      /* Override VS Code theme for consistent look */
      --vscode-sideBar-background: var(--bg-base);
    }
    body.cat-hidden {
      padding-bottom: 12px;
    }

    /* Notification Feed - pills stacking from bottom */
    .notification-feed {
      position: fixed;
      bottom: 68px; /* Above cat footer */
      left: 12px;
      right: 12px;
      z-index: 200;
      display: flex;
      flex-direction: column;
      gap: 6px;
      pointer-events: none;
    }
    .notification-pill {
      display: flex;
      align-items: center;
      gap: 6px;
      background: var(--bg-surface);
      border: 1px solid var(--accent);
      border-radius: 20px;
      padding: 6px 12px;
      font-size: 10px;
      opacity: 0;
      transform: translateY(20px);
      animation: pill-in 0.3s ease forwards;
      pointer-events: auto;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    }
    .notification-pill.removing {
      animation: pill-out 0.3s ease forwards;
    }
    @keyframes pill-in {
      to { opacity: 1; transform: translateY(0); }
    }
    @keyframes pill-out {
      to { opacity: 0; transform: translateY(-10px); }
    }
    .notification-pill .icon { width: 12px; height: 12px; flex-shrink: 0; }
    .notification-pill.success { border-color: var(--accent); }
    .notification-pill.success .icon { color: var(--accent); }
    .notification-pill.error { border-color: var(--error); }
    .notification-pill.error .icon { color: var(--error); }
    .notification-pill .pill-text { flex: 1; color: var(--text-primary); }
    .notification-pill .pill-time { color: var(--text-muted); margin-left: auto; }

    .spinner {
      width: 14px; height: 14px;
      border: 2px solid transparent;
      border-top-color: currentColor;
      border-radius: 50%;
      animation: spin 0.7s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }

    /* Section */
    .section { margin-bottom: 8px; }
    .section-header {
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 10px;
      font-weight: 600;
      color: var(--text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.5px;
      padding: 6px 8px;
      margin-bottom: 4px;
      cursor: pointer;
      border-radius: var(--radius-sm);
      transition: background 0.15s, color 0.15s;
    }
    .section-header:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
    .section-header .icon { width: 12px; height: 12px; opacity: 0.7; }
    .section-header .section-label { flex: 1; }
    .section-header .count {
      background: var(--bg-elevated);
      padding: 2px 6px;
      border-radius: 10px;
      font-size: 9px;
    }
    .section-header .chevron {
      width: 14px;
      height: 14px;
      opacity: 0.5;
      transition: transform 0.2s;
    }
    .section-header.collapsed .chevron {
      transform: rotate(-90deg);
    }
    .section-header .header-status-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: var(--text-muted);
      flex-shrink: 0;
    }
    .section-header .header-status-dot.on {
      background: var(--success);
      box-shadow: 0 0 6px var(--success);
    }
    .section-header .header-status-dot.connecting {
      background: var(--warning);
      animation: pulse 1s infinite;
    }
    .section-content {
      display: none;
      padding: 0 4px;
    }
    .section-content.visible {
      display: block;
    }

    /* Studio Card */
    .studio-card {
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius);
      padding: 12px;
      margin-bottom: 8px;
      transition: border-color 0.15s;
    }
    .studio-card:hover { border-color: var(--border-focus); }
    .studio-card.linked {
      border-color: var(--success);
      background: linear-gradient(135deg, var(--bg-surface) 0%, var(--success-soft) 100%);
    }

    .studio-header {
      display: flex;
      align-items: flex-start;
      gap: 10px;
      margin-bottom: 10px;
    }
    .studio-icon {
      width: 32px; height: 32px;
      background: var(--bg-elevated);
      border-radius: var(--radius-sm);
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }
    .studio-icon .icon { width: 18px; height: 18px; opacity: 0.8; }
    .studio-icon.linked { background: var(--success); }
    .studio-icon.linked .icon { opacity: 1; color: #fff; }

    .studio-info { flex: 1; min-width: 0; }
    .studio-name {
      font-weight: 600;
      font-size: 13px;
      margin-bottom: 2px;
      display: flex;
      align-items: center;
      gap: 6px;
    }
    .studio-name .badge {
      font-size: 9px;
      font-weight: 600;
      padding: 2px 5px;
      border-radius: 4px;
      background: var(--success);
      color: #fff;
      text-transform: uppercase;
      letter-spacing: 0.3px;
    }
    .studio-name .badge.unlinked {
      background: var(--text-secondary);
      opacity: 0.7;
    }
    .studio-meta {
      font-size: 10px;
      color: var(--text-secondary);
      font-family: var(--vscode-editor-font-family, monospace);
    }
    .studio-path {
      font-size: 10px;
      color: var(--text-muted);
      margin-top: 2px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .studio-actions {
      display: flex;
      gap: 6px;
      margin-top: 10px;
      padding-top: 10px;
      border-top: 1px solid var(--border);
    }
    .studio-btn {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 4px;
      padding: 6px 8px;
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      color: var(--text-primary);
      font-family: inherit;
      font-size: 10px;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.15s;
    }
    .studio-btn:hover { background: var(--bg-hover); border-color: var(--border-light); }
    .studio-btn .icon { width: 12px; height: 12px; }

    /* Sync, Extract, Test = Secondary buttons (gray) when not linked */
    .studio-btn.sync:hover,
    .studio-btn.extract:hover,
    .studio-btn.test:hover { border-color: var(--border-light); color: var(--text-primary); }

    /* Sync = Primary button (solid green) when linked */
    .studio-card.linked .studio-btn.sync {
      background: var(--accent);
      border-color: var(--accent);
      color: var(--bg-base);
    }
    .studio-card.linked .studio-btn.sync:hover {
      background: var(--accent-hover);
      border-color: var(--accent-hover);
      color: var(--bg-base);
    }
    .studio-card.linked .studio-btn.sync .icon { opacity: 1; }

    /* Extract = Blue primary button when linked */
    .studio-card.linked .studio-btn.extract {
      background: var(--blue);
      border-color: var(--blue);
      color: var(--bg-base);
    }
    .studio-card.linked .studio-btn.extract:hover {
      background: #7BB8FC;
      border-color: #7BB8FC;
      color: var(--bg-base);
    }
    .studio-card.linked .studio-btn.extract .icon { opacity: 1; }

    /* Link/Unlink buttons */
    .studio-btn.link { background: var(--success-soft); border-color: var(--success); color: var(--success); }
    .studio-btn.link:hover { background: var(--success); color: #fff; }
    .studio-btn.unlink { background: var(--warning-soft); border-color: var(--warning); color: var(--warning); }
    .studio-btn.unlink:hover { background: var(--warning); color: #fff; }

    /* Disabled buttons (when card not linked) */
    .studio-btn:disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }
    .studio-btn:disabled:hover {
      background: var(--bg-surface);
      border-color: var(--border);
      color: var(--text-primary);
    }

    /* Operation Status */
    .studio-status {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 6px 8px;
      margin-top: 8px;
      border-radius: var(--radius-sm);
      font-size: 10px;
      font-weight: 500;
    }
    .studio-status.running {
      background: var(--blue-soft);
      color: var(--blue);
    }
    .studio-status.success {
      background: var(--success-soft);
      color: var(--success);
    }
    .studio-status.error {
      background: var(--error-soft);
      color: var(--error);
    }
    .studio-status .spinner {
      width: 10px; height: 10px;
    }
    .studio-status .time {
      margin-left: auto;
      opacity: 0.7;
    }

    /* Empty State */
    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      padding: 16px 12px;
      color: var(--text-muted);
      text-align: center;
    }
    .empty-state .icon {
      width: 32px; height: 32px;
      opacity: 0.4;
      flex-shrink: 0;
    }
    .empty-state .empty-text {
      flex: 1;
    }
    .empty-state h3 {
      font-size: 12px;
      font-weight: 500;
      color: var(--text-secondary);
      margin-bottom: 2px;
    }
    .empty-state p {
      font-size: 11px;
      color: var(--text-muted);
      opacity: 0.8;
    }

    /* Server Control */
    .server-bar {
      display: flex;
      align-items: center;
      gap: 10px;
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius);
      padding: 10px 12px;
    }
    .server-status {
      display: flex;
      align-items: center;
      gap: 8px;
      flex: 1;
    }
    .status-dot {
      width: 8px; height: 8px;
      border-radius: 50%;
      background: var(--text-muted);
    }
    .status-dot.on { background: var(--success); box-shadow: 0 0 8px var(--success); }
    .status-dot.connecting { background: var(--warning); animation: pulse 1s infinite; }
    @keyframes pulse {
      0%, 100% { opacity: 1; }
      50% { opacity: 0.5; }
    }
    .server-label { font-size: 11px; font-weight: 500; }
    .server-btn {
      padding: 5px 10px;
      border-radius: var(--radius-sm);
      border: none;
      font-family: inherit;
      font-size: 10px;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.15s;
    }
    .server-btn.start { background: var(--success); color: #fff; }
    .server-btn.start:hover { filter: brightness(1.1); }
    .server-btn.stop { background: var(--error); color: #fff; }
    .server-btn.stop:hover { filter: brightness(1.1); }
    .server-btn:disabled { opacity: 0.5; cursor: not-allowed; }

    /* Quick Actions */
    .quick-row {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 10px;
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      margin-bottom: 6px;
      cursor: pointer;
      transition: all 0.15s;
      font-size: 11px;
    }
    .quick-row:hover { background: var(--bg-hover); border-color: var(--border-focus); }
    .quick-row .icon { width: 14px; height: 14px; opacity: 0.7; flex-shrink: 0; }
    .quick-row .label { flex: 1; }
    .quick-row .shortcut {
      font-size: 9px;
      color: var(--text-muted);
      background: var(--bg-elevated);
      padding: 2px 5px;
      border-radius: 3px;
      font-family: var(--vscode-editor-font-family, monospace);
    }
    .quick-row .toggle {
      width: 28px; height: 16px;
      background: var(--text-muted);
      border-radius: 8px;
      position: relative;
      transition: background 0.2s;
    }
    .quick-row .toggle.on { background: var(--blue); }  /* Blue for toggles, not green */
    .quick-row .toggle::after {
      content: '';
      position: absolute;
      top: 2px; left: 2px;
      width: 12px; height: 12px;
      background: #fff;
      border-radius: 50%;
      transition: transform 0.2s;
    }
    .quick-row .toggle.on::after { transform: translateX(12px); }
    .quick-row .arrow { width: 12px; height: 12px; opacity: 0.4; flex-shrink: 0; }

    .hidden { display: none !important; }

    /* Zen Cat Mascot - Fixed at bottom */
    .zen-cat-container {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      padding: 12px;
      height: 72px;
      cursor: pointer;
      user-select: none;
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      background: var(--bg-base);
      z-index: 100;
      border-top: 1px solid var(--border);
      box-shadow: 0 -4px 12px rgba(0, 0, 0, 0.3);
    }
    /* Spacer to push content above fixed cat */
    .cat-spacer {
      height: 72px;
    }
    .zen-cat-container:active .zen-cat {
      transform: scale(0.95);
    }
    .zen-cat-container.wiggle .zen-cat {
      animation: wiggle 0.3s ease infinite;
    }
    .zen-cat-container.pulse .zen-cat {
      animation: pulse-glow 1s ease infinite;
    }
    @keyframes wiggle {
      0%, 100% { transform: rotate(-3deg); }
      50% { transform: rotate(3deg); }
    }
    @keyframes pulse-glow {
      0%, 100% { opacity: 1; }
      50% { opacity: 0.7; }
    }
    .zen-cat-wrapper {
      display: flex;
      flex-direction: column;
      align-items: center;
      flex-shrink: 0;
      width: 52px;
    }
    .zen-cat {
      font-family: var(--vscode-editor-font-family, monospace);
      font-size: 9px;
      line-height: 1.1;
      white-space: pre;
      flex-shrink: 0;
      transition: color 0.3s ease, transform 0.15s ease;
    }
    .zen-cat-status {
      font-family: var(--vscode-editor-font-family, monospace);
      font-size: 8px;
      color: inherit;
      margin-top: 2px;
      opacity: 0.6;
      transition: color 0.3s ease, opacity 0.3s ease;
      white-space: nowrap;
      text-align: center;
    }
    .sink-name {
      color: inherit;
      font-weight: 600;
    }
    /* Cat mood colors - applied to wrapper so status inherits */
    .zen-cat-wrapper { color: #a78bfa; }
    .zen-cat.idle ~ .zen-cat-status { color: #a78bfa; }
    .zen-cat.syncing { animation: cat-bounce 0.5s ease infinite; }
    .zen-cat.syncing ~ .zen-cat-status { color: #60a5fa; }
    .zen-cat.success ~ .zen-cat-status { color: #4ade80; }
    .zen-cat.error ~ .zen-cat-status { color: #f87171; }
    .zen-cat.idle { color: #a78bfa; }
    .zen-cat.syncing { color: #60a5fa; }
    .zen-cat.success { color: #4ade80; }
    .zen-cat.error { color: #f87171; }
    @keyframes cat-bounce {
      0%, 100% { transform: translateY(0); }
      50% { transform: translateY(-2px); }
    }
    /* Speech bubble */
    .zen-quote-feed {
      position: relative;
      padding-left: 8px;
      flex: 1;
      min-width: 0;
    }
    .zen-quote {
      --bubble-bg: var(--bg-surface);
      --bubble-border: var(--border);
      display: block;
      width: fit-content;
      max-width: 100%;
      font-size: 11px;
      color: var(--text-secondary);
      font-style: italic;
      background: var(--bubble-bg);
      border: 1px solid var(--bubble-border);
      border-radius: 10px;
      padding: 6px 10px;
      position: relative;
      transition: border-color 0.3s ease, background 0.3s ease;
    }
    /* Thought bubble dots */
    .zen-quote::before,
    .zen-quote::after {
      content: '';
      position: absolute;
      border-radius: 50%;
      background: var(--bubble-bg);
      border: 1px solid var(--bubble-border);
      transition: background 0.3s ease, border-color 0.3s ease;
    }
    .zen-quote::before {
      width: 7px;
      height: 7px;
      left: -13px;
      top: 35%;
      transform: translateY(-50%);
      opacity: 1;
    }
    .zen-quote::after {
      width: 5px;
      height: 5px;
      left: -23px;
      top: 50%;
      transform: translateY(-50%);
      opacity: 1;
    }

    /* Typing animation - crisp, terminal-style appearance */
    /* Matches the static ASCII cat aesthetic */

    .zen-quote.typing::after {
      animation: dot-appear 0.15s steps(2, end) forwards;
    }
    .zen-quote.typing::before {
      animation: dot-appear 0.15s steps(2, end) 0.1s forwards;
    }
    .zen-quote.typing {
      animation: bubble-appear 0.2s steps(3, end) 0.18s both;
    }

    /* Dots snap into existence - no smooth scaling */
    @keyframes dot-appear {
      0% {
        transform: translateY(-50%) scale(0);
        opacity: 0;
      }
      100% {
        transform: translateY(-50%) scale(1);
        opacity: 1;
      }
    }

    /* Container fades in with slight step effect */
    @keyframes bubble-appear {
      0% {
        opacity: 0;
      }
      33% {
        opacity: 0.4;
      }
      66% {
        opacity: 0.8;
      }
      100% {
        opacity: 1;
      }
    }
    .zen-cat-container:hover .zen-quote {
      border-color: var(--border-light);
    }
    /* Typewriter cursor - inline span */
    .typing-cursor {
      animation: blink 0.7s infinite;
      margin-left: 1px;
    }
    @keyframes blink {
      0%, 50% { opacity: 1; }
      51%, 100% { opacity: 0; }
    }

    /* Collapsible Section */
    .collapsible-header {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 8px 10px;
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      cursor: pointer;
      transition: all 0.15s;
      margin-bottom: 6px;
    }
    .collapsible-header:hover { background: var(--bg-hover); border-color: var(--border-focus); }
    .collapsible-header .icon { width: 12px; height: 12px; opacity: 0.7; flex-shrink: 0; }
    .collapsible-header .label { flex: 1; font-size: 10px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-secondary); }
    .collapsible-header .chevron { width: 12px; height: 12px; opacity: 0.5; transition: transform 0.2s; }
    .collapsible-header.expanded .chevron { transform: rotate(90deg); }
    .collapsible-content { padding: 0 0 8px 0; display: none; }
    .collapsible-content.visible { display: block; }

    /* Settings Row */
    .setting-row {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 10px;
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      margin-bottom: 4px;
    }
    .setting-row .setting-label { flex: 1; font-size: 11px; color: var(--text-primary); }
    .setting-row .setting-value { font-size: 11px; color: var(--text-secondary); font-family: var(--vscode-editor-font-family, monospace); }

    /* Range Slider */
    .range-container { display: flex; align-items: center; gap: 8px; }
    .range-container input[type="range"] {
      -webkit-appearance: none;
      width: 80px;
      height: 4px;
      background: var(--bg-elevated);
      border-radius: 2px;
      outline: none;
    }
    .range-container input[type="range"]::-webkit-slider-thumb {
      -webkit-appearance: none;
      width: 12px;
      height: 12px;
      background: var(--blue);  /* Blue for sliders, not green */
      border-radius: 50%;
      cursor: pointer;
    }
    .range-value { font-size: 10px; color: var(--text-secondary); min-width: 20px; text-align: right; }

    /* Update Banner */
    .update-banner {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px 12px;
      background: linear-gradient(135deg, var(--blue-soft) 0%, var(--purple-soft) 100%);
      border: 1px solid var(--blue);
      border-radius: var(--radius);
      margin-bottom: 12px;
      animation: slideIn 0.3s ease;
    }
    .update-banner .icon { width: 16px; height: 16px; color: var(--blue); flex-shrink: 0; }
    .update-banner .text { flex: 1; font-size: 11px; color: var(--text-primary); }
    .update-banner .text strong { color: var(--blue); }
    .update-banner .dismiss {
      background: none;
      border: none;
      color: var(--text-muted);
      cursor: pointer;
      padding: 2px;
      font-size: 14px;
      line-height: 1;
    }
    .update-banner .dismiss:hover { color: var(--text-primary); }

    /* Server Stats */
    .server-stats {
      display: flex;
      gap: 12px;
      padding: 8px 0;
      font-size: 10px;
      color: var(--text-muted);
    }
    .server-stat { display: flex; align-items: center; gap: 4px; }
    .server-stat .icon { width: 10px; height: 10px; opacity: 0.7; }

  </style>
</head>
<body>
  <!-- Update Banner -->
  <div class="update-banner hidden" id="updateBanner">
    <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
    <span class="text"><strong>v<span id="updateVersion"></span></strong> available</span>
    <button class="dismiss" id="dismissUpdate">×</button>
  </div>

  <!-- Notification Feed -->
  <div class="notification-feed" id="notificationFeed"></div>

  <!-- Studios Section -->
  <div class="section">
    <div class="section-header" data-section="studios">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
      <span class="section-label">Studios</span>
      <span class="count" id="studioCount">0</span>
      <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 9l6 6 6-6"/></svg>
    </div>
    <div class="section-content visible" id="studiosContent">
      <div id="studioList"></div>
      <div class="empty-state" id="emptyState">
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
        <div class="empty-text">
          <h3 id="emptyTitle">No Studios Connected</h3>
          <p id="emptyDesc">Open Roblox Studio and install the RbxSync plugin</p>
        </div>
      </div>
    </div>
  </div>

  <!-- Server Section -->
  <div class="section">
    <div class="section-header" data-section="server">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><circle cx="6" cy="6" r="1" fill="currentColor"/><circle cx="6" cy="18" r="1" fill="currentColor"/></svg>
      <span class="section-label">Server</span>
      <div class="header-status-dot" id="headerServerDot"></div>
      <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 9l6 6 6-6"/></svg>
    </div>
    <div class="section-content visible" id="serverContent">
      <div class="server-bar">
        <div class="server-status">
          <div class="status-dot" id="serverDot"></div>
          <span class="server-label" id="serverLabel">Stopped</span>
        </div>
        <button class="server-btn start" id="serverBtn">Start</button>
      </div>
    </div>
  </div>

  <!-- Tools Section -->
  <div class="section">
    <div class="section-header collapsed" data-section="tools">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.7 6.3a1 1 0 000 1.4l1.6 1.6a1 1 0 001.4 0l3.77-3.77a6 6 0 01-7.94 7.94l-6.91 6.91a2.12 2.12 0 01-3-3l6.91-6.91a6 6 0 017.94-7.94l-3.76 3.76z"/></svg>
      <span class="section-label">Tools</span>
      <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 9l6 6 6-6"/></svg>
    </div>
    <div class="section-content" id="toolsContent">
      <div class="quick-row" id="consoleBtn">
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
        <span class="label">Console</span>
        <svg class="arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18l6-6-6-6"/></svg>
      </div>
      <div class="quick-row" id="e2eBtn">
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
        <span class="label">E2E Mode</span>
        <div class="toggle" id="e2eToggle"></div>
      </div>
      <div class="quick-row" id="rbxjsonBtn">
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
        <span class="label">Show .rbxjson</span>
        <div class="toggle" id="rbxjsonToggle"></div>
      </div>
    </div>
  </div>

  <!-- Cat Spacer (pushes content above fixed cat footer) -->
  <div class="cat-spacer" id="catSpacer"></div>

  <!-- Zen Cat Mascot (Footer) -->
  <div class="zen-cat-container" id="zenCat">
    <div class="zen-cat-wrapper">
      <div class="zen-cat idle" id="zenCatArt"></div>
      <div class="zen-cat-status" id="zenCatStatus">~ Napping...</div>
    </div>
    <div class="zen-quote-feed">
      <div class="zen-quote" id="zenQuote"></div>
    </div>
  </div>

  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let state = null;

    // Zen Cat ASCII Art for different moods (detailed 4-line)
    const CAT_ART = {
      idle: \` /\\\\_/\\\\
( -.- )
 />♡<\\\\
  ~z~\`,
      syncing: \` /\\\\_/\\\\
( o.o )
 />~<\\\\
  ~~~\`,
      success: \` /\\\\_/\\\\
( ^.^ )
 />v<\\\\
*purr*\`,
      error: \` /\\\\_/\\\\
( >.< )
 />!<\\\\
  ?!?\`
    };

    // Status indicator text for different moods
    const CAT_STATUS = {
      idle: ['~ Napping...', '◦ Dreaming...', '✧ Resting...', '~ Dozing...'],
      syncing: ['✻ Working...', '◈ Syncing...', '⟳ Pushing...', '✦ Busy...'],
      success: ['♡ Happy!', '✿ Purrfect!', '★ Done!', '❋ Yay!'],
      error: ['⚠ Oh no...', '✕ Uh oh...', '◇ Oops...'],
      thinking: ['✻ Thinking...', '◦ Pondering...', '✧ Hmm...'],
      connecting: ['◈ Waking...', '✦ Starting...', '⟳ Connecting...'],
    };

    function updateCatStatus(mood) {
      const statusEl = document.getElementById('zenCatStatus');
      if (!statusEl) return;
      const statuses = CAT_STATUS[mood] || CAT_STATUS.idle;
      statusEl.textContent = statuses[Math.floor(Math.random() * statuses.length)];
    }

    // Talking cat faces for animation while typing
    const CAT_TALK = [
      \` /\\\\_/\\\\
( °o° )
 />~<\\\\
  ...\`,
      \` /\\\\_/\\\\
( °-° )
 />~<\\\\
  ...\`
    ];

    // Contextual cat messages by operation type
    const CAT_MESSAGES = {
      sync: [
        "Syncing meow~",
        "Pushing changes...",
        "Almost there~",
        "Uploading scripts...",
        "Just a moment..."
      ],
      extract: [
        "Extracting meow~",
        "Grabbing files...",
        "So many instances!",
        "Downloading...",
        "Fetching data~"
      ],
      test: [
        "Testing meow~",
        "Running checks...",
        "Paws crossed!",
        "Executing...",
        "Let's see~"
      ],
      success: [
        "All done! Purr~",
        "Meow-velous!",
        "Nailed it!",
        "Purrfect!",
        "Success meow~"
      ],
      error: [
        "Oh no meow!",
        "Something went wrong...",
        "Hiss!",
        "Uh oh...",
        "Error meow!"
      ],
      serverStart: [
        "Server starting~",
        "Waking up...",
        "Stretching...",
        "Coming online!"
      ],
      serverConnected: [
        "Connected! Purr~",
        "Ready to go!",
        "All systems meow!",
        "Online and cozy~"
      ],
      serverStopped: [
        "Server stopped~",
        "Taking a nap...",
        "Zzz...",
        "Going offline~"
      ],
      studioJoined: [
        "New friend!",
        "Hello Studio~",
        "Welcome meow!",
        "A visitor!"
      ],
      studioLeft: [
        "Bye bye~",
        "Studio left...",
        "See you later!",
        "Gone meow..."
      ],
      studioLinked: [
        "Linked up!",
        "Connected meow~",
        "We're bonded!",
        "Link established!"
      ],
      studioUnlinked: [
        "Unlinked~",
        "Disconnected...",
        "Link broken",
        "Separated meow"
      ]
    };

    // Greeting messages for first launch - high priority welcome
    const CAT_GREETINGS = [
      "Welcome back, friend!",
      "Hello there! Ready to code?",
      "Hi! Sink is here to help~",
      "Welcome to RbxSync!",
      "Hey! Let's build something!",
      "Good to see you!",
      "Sink reporting for duty!",
      "Hello, developer!",
      "Ready when you are~",
      "Let's make something cool!",
      "Sink is happy to see you!",
      "Welcome! *purrs*",
      "Hey friend! Let's go~",
      "Hi hi! Ready to sync?",
      "Sink says hello!"
    ];

    // Cat click reactions - fun cat things to say
    const CAT_CLICK_MESSAGES = [
      "Meow!",
      "Mrrp?",
      "Purrr~",
      "*blinks slowly*",
      "Nya~",
      "*head bonk*",
      "Pet me more!",
      "*kneads paws*",
      "Prrrrt!",
      "*tail swish*",
      "Feed me?",
      "*chirp chirp*",
      "So sleepy...",
      "*stretches*",
      "Mrow!",
      "*flops over*",
      "Treats?",
      "*ear twitch*",
      "Mew~",
      "*purrs loudly*"
    ];

    // Fun colors for cat clicks
    const CAT_CLICK_COLORS = [
      '#f472b6', // pink
      '#a78bfa', // purple
      '#60a5fa', // blue
      '#4ade80', // green
      '#facc15', // yellow
      '#fb923c', // orange
      '#f87171', // red
      '#2dd4bf', // teal
    ];

    // Zen wisdom quotes (short - max 2 lines)
    const ZEN_QUOTES = [
      "Just breathe.",
      "Be here now.",
      "There is only Now.",
      "Breathe and let be.",
      "Pay attention.",
      "Attention!",
      "What am I?",
      "Om mani padme hum",
      "When hungry, eat.",
      "When tired, sleep.",
      "You are the sky.",
      "YOLO",
      "Who is breathing?",
      "Focus on your breath.",
      "Let go of thinking.",
      "Just be in this moment.",
      "Do one thing at a time.",
      "Do dishes, rake leaves.",
      "Chop wood, carry water.",
      "What you think, you become.",
      "Nothing is permanent.",
      "Seek the mind.",
      "You are awareness.",
      "There is only the Present.",
      "Clean the floor with love.",
      "Drink your tea slowly.",
      "Attend the moment.",
      "We have only now.",
      "Do not dwell in the past.",
      "Do not dream of the future.",
      "My religion is love.",
      "Listen to your heart.",
      "If not now, when?",
      "Get the inside right.",
      "Give your fullest attention.",
      "Gate, gate, paragate...",
      "Let come what comes.",
      "Let go what goes.",
      "See what remains.",
      "Each time for the first time.",
      "You need nothing more.",
      "I am detached.",
      "There is only light.",
      "Your thoughts come and go.",
      "Express yourself as you are.",
      "Breathe in deeply.",
      "Breathe out slowly.",
      "Feel what you feel now.",
      "Put it all down.",
      "Let it all go.",
      "We're present now.",
      "Be kind whenever possible.",
      "Nothing else matters now.",
      "Become still and alert."
    ];

    // Sink's personal thoughts (short - max 2 lines)
    const SINK_QUOTES = [
      "Sink is hungry...",
      "Sink believes in you~",
      "Sink had a nice nap.",
      "Sink is proud of you!",
      "Sink loves a good sync.",
      "Sink feels cozy today.",
      "Sink sends good vibes~",
      "Sink is happy here.",
      "Sink dreams of treats.",
      "Sink is cheering you on!",
      "Sink's heart is full.",
      "Sink feels lucky today.",
      "Sink knows you got this!",
      "Sink enjoys the typing.",
      "Sink is here for you.",
      "Sink thinks you're great!",
      "Sink is contemplating...",
      "Sink appreciates you.",
      "Sink is grateful~",
      "Sink wonders about meow.",
      "Sink needs more naps.",
      "Sink likes this code.",
      "Sink is vibing~",
      "Sink says keep going!",
      "Sink found a sunbeam."
    ];

    // Combine all quotes
    const ALL_QUOTES = [...ZEN_QUOTES, ...SINK_QUOTES];

    // Initialize zen cat quote feed
    let quoteIndex = 0;
    let shuffledQuotes = [];
    let typewriterTimeout = null;
    let currentTypewriterText = '';
    let isTyping = false;
    let lastOperationType = null;
    let operationMessageIndex = 0;
    let lastConnectionStatus = null;
    let priorityMessageUntil = 0; // timestamp when priority message expires
    let lastPlaceCount = 0;
    let lastLinkedPlaceIds = new Set();
    let lastNotificationTime = null;

    // Notification feed - add pills that stack from bottom
    function addNotification(text, success, time) {
      const feed = document.getElementById('notificationFeed');
      if (!feed) return;

      const pill = document.createElement('div');
      pill.className = 'notification-pill ' + (success ? 'success' : 'error');
      pill.innerHTML = \`
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          \${success
            ? '<path d="M20 6L9 17l-5-5"/>'
            : '<circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/>'}
        </svg>
        <span class="pill-text">\${text}</span>
        <span class="pill-time">\${time}</span>
      \`;

      feed.appendChild(pill);

      // Remove after 5 seconds
      setTimeout(() => {
        pill.classList.add('removing');
        setTimeout(() => pill.remove(), 300);
      }, 5000);

      // Keep max 3 notifications
      while (feed.children.length > 3) {
        feed.firstChild.remove();
      }
    }

    // Typewriter effect - types text character by character
    let talkInterval = null;

    // Get current cat color based on mood
    function getCatColor() {
      const catEl = document.getElementById('zenCatArt');
      if (catEl) {
        const style = window.getComputedStyle(catEl);
        return style.color;
      }
      return '#a78bfa'; // default idle color
    }

    function typewriterEffect(text, element, speed = 40) {
      // Clear any existing typewriter
      if (typewriterTimeout) {
        clearTimeout(typewriterTimeout);
      }
      if (talkInterval) {
        clearInterval(talkInterval);
        talkInterval = null;
      }

      currentTypewriterText = text;
      isTyping = true;
      element.classList.add('typing');
      element.innerHTML = '<span class="typing-cursor">|</span>';
      updateCatStatus('thinking');

      // Start cat talking animation
      const catEl = document.getElementById('zenCatArt');
      let talkFrame = 0;
      if (catEl && !isClickAnimating) {
        talkInterval = setInterval(() => {
          catEl.textContent = CAT_TALK[talkFrame % 2];
          talkFrame++;
        }, 150);
      }

      let i = 0;
      let displayText = '';

      function typeChar() {
        if (i < text.length && currentTypewriterText === text) {
          displayText += text.charAt(i);
          // Highlight "Sink" with cat's color, add cursor at end
          const catColor = getCatColor();
          const highlighted = displayText.replace(/Sink/g, '<span class="sink-name" style="color:' + catColor + '">Sink</span>');
          element.innerHTML = highlighted + '<span class="typing-cursor">|</span>';
          i++;
          typewriterTimeout = setTimeout(typeChar, speed);
        } else {
          isTyping = false;
          element.classList.remove('typing');
          // Remove cursor when done, show final text
          const catColor = getCatColor();
          element.innerHTML = displayText.replace(/Sink/g, '<span class="sink-name" style="color:' + catColor + '">Sink</span>');
          // Stop talking animation and restore cat
          if (talkInterval) {
            clearInterval(talkInterval);
            talkInterval = null;
          }
          if (catEl && !isClickAnimating) {
            updateCatMood(state?.catMood || 'idle');
          }
        }
      }
      typeChar();
    }

    // Get random message for operation type
    function getOperationMessage(opType) {
      const messages = CAT_MESSAGES[opType];
      if (!messages || messages.length === 0) return '';
      return messages[Math.floor(Math.random() * messages.length)];
    }

    // Show a priority message (operations, server status, clicks)
    function showPriorityMessage(text, element, duration = 5000) {
      priorityMessageUntil = Date.now() + duration;
      lastQuoteTime = Date.now() + duration; // Reset quote timer to after priority expires
      typewriterEffect(text, element, 35);
    }

    // Check if we can show a low-priority zen quote
    function canShowZenQuote() {
      if (Date.now() < priorityMessageUntil) return false;
      if (isClickAnimating) return false;
      if (state?.catMood && state.catMood !== 'idle') return false;
      return true;
    }

    let lastQuoteTime = 0;

    let isFirstLaunch = true;
    let greetingUntil = 0; // Timestamp when greeting protection expires

    function initZenCat() {
      const quoteEl = document.getElementById('zenQuote');
      if (!quoteEl) return;

      // Shuffle quotes for variety (mix zen + Sink quotes)
      shuffledQuotes = [...ALL_QUOTES].sort(() => Math.random() - 0.5);

      // Show greeting on first launch with high priority (8 seconds)
      // Also set greetingUntil to block server status messages during greeting
      if (isFirstLaunch) {
        const greeting = CAT_GREETINGS[Math.floor(Math.random() * CAT_GREETINGS.length)];
        greetingUntil = Date.now() + 8000;
        showPriorityMessage(greeting, quoteEl, 8000);
        isFirstLaunch = false;
      } else {
        typewriterEffect(shuffledQuotes[0], quoteEl);
      }
      lastQuoteTime = Date.now();

      // Check every 2 seconds if we should show a new quote (10s after last activity)
      setInterval(() => {
        const quoteEl = document.getElementById('zenQuote');
        if (!quoteEl) return;

        const now = Date.now();
        const timeSinceLastQuote = now - lastQuoteTime;
        const timeSincePriority = now - priorityMessageUntil;

        // Show new quote if: can show zen quote AND (10s since last quote OR priority just expired)
        if (canShowZenQuote() && timeSinceLastQuote >= 10000 && timeSincePriority >= 0) {
          quoteIndex = (quoteIndex + 1) % shuffledQuotes.length;
          typewriterEffect(shuffledQuotes[quoteIndex], quoteEl);
          lastQuoteTime = now;
        }
      }, 2000);

      // Update cat art
      updateCatMood('idle');
    }

    function updateCatMood(mood) {
      const catEl = document.getElementById('zenCatArt');
      if (!catEl) return;

      catEl.textContent = CAT_ART[mood] || CAT_ART.idle;
      catEl.className = 'zen-cat ' + mood;
      updateCatStatus(mood);
    }

    // Cat click reaction
    let isClickAnimating = false;
    function onCatClick() {
      const catEl = document.getElementById('zenCatArt');
      const quoteEl = document.getElementById('zenQuote');
      const container = document.getElementById('zenCat');
      if (!catEl || isClickAnimating) return;

      isClickAnimating = true;

      // Pick random color
      const randomColor = CAT_CLICK_COLORS[Math.floor(Math.random() * CAT_CLICK_COLORS.length)];

      // Show surprised face (4-line version)
      catEl.textContent = \` /\\\\_/\\\\
( O.O )
 />!<\\\\
 meow!\`;
      catEl.style.color = randomColor;

      // Add wiggle animation to container, color to speech bubble
      if (container) {
        container.classList.add('wiggle');
      }
      if (quoteEl) {
        quoteEl.style.borderColor = randomColor;
        quoteEl.style.setProperty('--bubble-bg', randomColor + '40');
        quoteEl.style.setProperty('--bubble-border', randomColor);
      }

      // Show cat message with priority
      if (quoteEl) {
        const catMsg = CAT_CLICK_MESSAGES[Math.floor(Math.random() * CAT_CLICK_MESSAGES.length)];
        showPriorityMessage(catMsg, quoteEl, 4000);
      }

      // Return to normal after longer delay
      setTimeout(() => {
        updateCatMood(state?.catMood || 'idle');
        if (container) {
          container.classList.remove('wiggle');
        }
        if (quoteEl) {
          quoteEl.style.borderColor = '';
          quoteEl.style.setProperty('--bubble-bg', '');
          quoteEl.style.setProperty('--bubble-border', '');
        }
        isClickAnimating = false;
        // Zen quote will naturally cycle after 45 seconds
      }, 2000);
    }

    // Initialize on load
    initZenCat();

    // Add click handler to cat
    document.getElementById('zenCat')?.addEventListener('click', onCatClick);

    window.addEventListener('message', e => {
      if (e.data.type === 'stateUpdate') {
        state = e.data.state;
        render(state);
      }
    });

    function render(s) {
      // Update zen cat mood and message
      const catMood = s.catMood || 'idle';
      const opType = s.catOperationType;
      const quoteEl = document.getElementById('zenQuote');
      const catEl = document.getElementById('zenCatArt');
      const container = document.getElementById('zenCat');
      const connStatus = s.connectionStatus;

      // Check for server connection status changes (skip during greeting)
      if (connStatus !== lastConnectionStatus && !isClickAnimating && Date.now() > greetingUntil) {
        if (connStatus === 'connecting' && lastConnectionStatus !== 'connecting') {
          // Server starting - show alert cat with animation
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( o.o )
 />~<\\\\
  ~~~\`;
            catEl.style.color = '#facc15';
          }
          updateCatStatus('connecting');
          if (container) {
            container.classList.add('pulse');
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#facc15';
            quoteEl.style.setProperty('--bubble-bg', '#facc1540');
            quoteEl.style.setProperty('--bubble-border', '#facc15');
            const msg = getOperationMessage('serverStart');
            showPriorityMessage(msg, quoteEl, 5000);
          }
        } else if (connStatus === 'connected' && lastConnectionStatus !== 'connected') {
          // Server connected - show happy cat
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( ^.^ )
 />v<\\\\
*purr*\`;
            catEl.style.color = '#4ade80';
          }
          updateCatStatus('success');
          if (container) {
            container.classList.remove('pulse');
            container.classList.add('wiggle');
            setTimeout(() => container.classList.remove('wiggle'), 1000);
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#4ade80';
            quoteEl.style.setProperty('--bubble-bg', '#4ade8040');
            quoteEl.style.setProperty('--bubble-border', '#4ade80');
            const msg = getOperationMessage('serverConnected');
            showPriorityMessage(msg, quoteEl, 5000);
          }
          // Reset cat after delay
          setTimeout(() => {
            if (catEl) catEl.style.color = '';
            if (quoteEl) {
              quoteEl.style.borderColor = '';
              quoteEl.style.setProperty('--bubble-bg', '');
              quoteEl.style.setProperty('--bubble-border', '');
            }
            updateCatMood('idle');
          }, 3000);
        } else if (connStatus === 'disconnected' && lastConnectionStatus && lastConnectionStatus !== 'disconnected') {
          // Server stopped - show sleepy cat
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( -.- )
 />♡<\\\\
  ~z~\`;
            catEl.style.color = '#a78bfa';
          }
          if (container) {
            container.classList.remove('pulse');
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#a78bfa';
            quoteEl.style.setProperty('--bubble-bg', '#a78bfa40');
            quoteEl.style.setProperty('--bubble-border', '#a78bfa');
            const msg = getOperationMessage('serverStopped');
            showPriorityMessage(msg, quoteEl, 5000);
          }
          // Reset after delay
          setTimeout(() => {
            if (catEl) catEl.style.color = '';
            if (quoteEl) {
              quoteEl.style.borderColor = '';
              quoteEl.style.setProperty('--bubble-bg', '');
              quoteEl.style.setProperty('--bubble-border', '');
            }
          }, 3000);
        }
        lastConnectionStatus = connStatus;
      } else if (Date.now() <= greetingUntil) {
        // During greeting, still track status but don't show messages
        lastConnectionStatus = connStatus;
      } else if (!isClickAnimating) {
        // Normal mood updates (when not reacting to server changes)
        updateCatMood(catMood);
      }

      // Check for studio count changes (new studios joining or leaving)
      const currentPlaceCount = s.places ? s.places.length : 0;
      if (currentPlaceCount !== lastPlaceCount && lastConnectionStatus === 'connected') {
        if (currentPlaceCount > lastPlaceCount && lastPlaceCount > 0) {
          // New studio joined
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( o.o )
 />~<\\\\
  !!!\`;
            catEl.style.color = '#60a5fa';
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#60a5fa';
            quoteEl.style.setProperty('--bubble-bg', '#60a5fa40');
            quoteEl.style.setProperty('--bubble-border', '#60a5fa');
            showPriorityMessage(getOperationMessage('studioJoined'), quoteEl, 4000);
          }
          setTimeout(() => {
            if (catEl) catEl.style.color = '';
            if (quoteEl) {
              quoteEl.style.borderColor = '';
              quoteEl.style.setProperty('--bubble-bg', '');
              quoteEl.style.setProperty('--bubble-border', '');
            }
            updateCatMood('idle');
          }, 3000);
        } else if (currentPlaceCount < lastPlaceCount) {
          // Studio left
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( ;.; )
 />~<\\\\
  ...\`;
            catEl.style.color = '#a78bfa';
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#a78bfa';
            quoteEl.style.setProperty('--bubble-bg', '#a78bfa40');
            quoteEl.style.setProperty('--bubble-border', '#a78bfa');
            showPriorityMessage(getOperationMessage('studioLeft'), quoteEl, 4000);
          }
          setTimeout(() => {
            if (catEl) catEl.style.color = '';
            if (quoteEl) {
              quoteEl.style.borderColor = '';
              quoteEl.style.setProperty('--bubble-bg', '');
              quoteEl.style.setProperty('--bubble-border', '');
            }
            updateCatMood('idle');
          }, 3000);
        }
        lastPlaceCount = currentPlaceCount;
      } else if (lastPlaceCount === 0) {
        lastPlaceCount = currentPlaceCount;
      }

      // Check for link/unlink changes
      const currentLinkedIds = new Set();
      if (s.places) {
        s.places.forEach(p => {
          if (p.project_dir === s.currentProjectDir && p.place_id) {
            currentLinkedIds.add(p.place_id);
          }
        });
      }
      // Check for newly linked
      for (const id of currentLinkedIds) {
        if (!lastLinkedPlaceIds.has(id) && lastLinkedPlaceIds.size > 0 || (lastLinkedPlaceIds.size === 0 && currentLinkedIds.size > 0 && lastPlaceCount > 0)) {
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( ^.^ )
 />v<\\\\
 yay!\`;
            catEl.style.color = '#4ade80';
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#4ade80';
            quoteEl.style.setProperty('--bubble-bg', '#4ade8040');
            quoteEl.style.setProperty('--bubble-border', '#4ade80');
            showPriorityMessage(getOperationMessage('studioLinked'), quoteEl, 4000);
          }
          setTimeout(() => {
            if (catEl) catEl.style.color = '';
            if (quoteEl) {
              quoteEl.style.borderColor = '';
              quoteEl.style.setProperty('--bubble-bg', '');
              quoteEl.style.setProperty('--bubble-border', '');
            }
            updateCatMood('idle');
          }, 3000);
          break;
        }
      }
      // Check for newly unlinked
      for (const id of lastLinkedPlaceIds) {
        if (!currentLinkedIds.has(id)) {
          if (catEl) {
            catEl.textContent = \` /\\\\_/\\\\
( -.- )
 />~<\\\\
  ok~\`;
            catEl.style.color = '#facc15';
          }
          if (quoteEl) {
            quoteEl.style.borderColor = '#facc15';
            quoteEl.style.setProperty('--bubble-bg', '#facc1540');
            quoteEl.style.setProperty('--bubble-border', '#facc15');
            showPriorityMessage(getOperationMessage('studioUnlinked'), quoteEl, 4000);
          }
          setTimeout(() => {
            if (catEl) catEl.style.color = '';
            if (quoteEl) {
              quoteEl.style.borderColor = '';
              quoteEl.style.setProperty('--bubble-bg', '');
              quoteEl.style.setProperty('--bubble-border', '');
            }
            updateCatMood('idle');
          }, 3000);
          break;
        }
      }
      lastLinkedPlaceIds = currentLinkedIds;

      // Show contextual messages during operations (only if not reacting to server)
      if (quoteEl && lastConnectionStatus === connStatus) {
        if (opType && catMood === 'syncing') {
          // Active operation - show contextual message (high priority)
          if (lastOperationType !== opType) {
            lastOperationType = opType;
            const msg = getOperationMessage(opType);
            showPriorityMessage(msg, quoteEl, 10000);
          }
        } else if (catMood === 'success') {
          // Success - show celebration message (high priority)
          if (lastOperationType !== 'success') {
            lastOperationType = 'success';
            const msg = getOperationMessage('success');
            showPriorityMessage(msg, quoteEl, 6000);
          }
        } else if (catMood === 'error') {
          // Error - show error message (high priority)
          if (lastOperationType !== 'error') {
            lastOperationType = 'error';
            const msg = getOperationMessage('error');
            showPriorityMessage(msg, quoteEl, 6000);
          }
        } else if (catMood === 'idle' && lastOperationType !== null) {
          // Returned to idle - only show zen quote if no priority message active
          lastOperationType = null;
          if (canShowZenQuote()) {
            quoteIndex = (quoteIndex + 1) % shuffledQuotes.length;
            typewriterEffect(shuffledQuotes[quoteIndex], quoteEl);
          }
        }
      }

      // Notification Feed
      if (s.lastResult && s.lastResult.time !== lastNotificationTime) {
        lastNotificationTime = s.lastResult.time;
        addNotification(s.lastResult.label, s.lastResult.success, relTime(s.lastResult.time));
      }

      // Server
      const isOn = s.connectionStatus === 'connected';
      const isConnecting = s.connectionStatus === 'connecting';
      document.getElementById('serverDot').className = 'status-dot' + (isOn ? ' on' : isConnecting ? ' connecting' : '');
      document.getElementById('headerServerDot').className = 'header-status-dot' + (isOn ? ' on' : isConnecting ? ' connecting' : '');
      document.getElementById('serverLabel').textContent = isOn ? 'Running' : isConnecting ? 'Starting...' : 'Stopped';
      const btn = document.getElementById('serverBtn');
      btn.textContent = isOn ? 'Stop' : isConnecting ? '...' : 'Start';
      btn.className = 'server-btn ' + (isOn ? 'stop' : 'start');
      btn.disabled = isConnecting;

      // Studios
      const list = document.getElementById('studioList');
      const empty = document.getElementById('emptyState');
      const count = document.getElementById('studioCount');

      list.innerHTML = '';
      count.textContent = s.places.length;

      if (s.places.length === 0) {
        empty.classList.remove('hidden');
        document.getElementById('emptyTitle').textContent = isOn ? 'No Studios Linked' : 'Server Not Running';
        document.getElementById('emptyDesc').textContent = isOn
          ? 'Open Studio and set the project path to this workspace'
          : 'Start the server to connect to Roblox Studio';
      } else {
        empty.classList.add('hidden');

        // Sort: linked first
        const sorted = [...s.places].sort((a, b) => {
          const aLinked = a.project_dir === s.currentProjectDir;
          const bLinked = b.project_dir === s.currentProjectDir;
          return bLinked - aLinked;
        });

        sorted.forEach((place, idx) => {
          const isLinked = place.project_dir === s.currentProjectDir;
          // Generate studioKey matching the server logic - prefer session_id
          const studioKey = place.session_id
            ? 'session_' + place.session_id
            : (place.place_id && place.place_id > 0)
              ? 'id_' + place.place_id
              : 'name_' + (place.place_name || 'unknown') + '_' + idx;
          const op = s.studioOperations[studioKey];
          const card = document.createElement('div');
          card.className = 'studio-card' + (isLinked ? ' linked' : '');

          let statusHtml = '';
          if (op) {
            const elapsed = op.endTime
              ? ((op.endTime - op.startTime) / 1000).toFixed(1) + 's'
              : relTime(op.startTime);
            statusHtml = \`
              <div class="studio-status \${op.status}">
                \${op.status === 'running' ? '<div class="spinner"></div>' : ''}
                <span>\${op.message}</span>
                <span class="time">\${elapsed}</span>
              </div>
            \`;
          }

          card.innerHTML = \`
            <div class="studio-header">
              <div class="studio-icon\${isLinked ? ' linked' : ''}">
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="2" y="3" width="20" height="14" rx="2"/>
                  <path d="M8 21h8M12 17v4"/>
                </svg>
              </div>
              <div class="studio-info">
                <div class="studio-name">
                  \${place.place_name || 'Unnamed Place'}
                  \${isLinked ? '<span class="badge">Linked</span>' : '<span class="badge unlinked">Unlinked</span>'}
                </div>
                <div class="studio-meta">ID: \${place.place_id || 'Unknown'}</div>
                <div class="studio-path" title="\${place.project_dir}">\${shortenPath(place.project_dir)}</div>
              </div>
            </div>
            \${statusHtml}
            <div class="studio-actions">
              <button class="studio-btn sync" data-action="sync" data-dir="\${place.project_dir}" data-place-id="\${place.place_id}" data-session-id="\${place.session_id || ''}" \${isLinked ? '' : 'disabled title="Link Studio to this project first"'}>
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
                  <polyline points="17 8 12 3 7 8"/>
                  <line x1="12" y1="3" x2="12" y2="15"/>
                </svg>
                Sync
              </button>
              <button class="studio-btn extract" data-action="extract" data-dir="\${place.project_dir}" data-place-id="\${place.place_id}" data-session-id="\${place.session_id || ''}" \${isLinked ? '' : 'disabled title="Link Studio to this project first"'}>
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
                  <polyline points="7 10 12 15 17 10"/>
                  <line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                Extract
              </button>
              <button class="studio-btn test" data-action="test" data-dir="\${place.project_dir}" data-place-id="\${place.place_id}" data-session-id="\${place.session_id || ''}" \${isLinked ? '' : 'disabled title="Link Studio to this project first"'}>
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polygon points="5 3 19 12 5 21 5 3"/>
                </svg>
                Test
              </button>
            </div>
          \`;
          list.appendChild(card);
        });

        // Attach action handlers
        list.querySelectorAll('.studio-btn').forEach(btn => {
          btn.onclick = () => {
            // Skip if button is disabled (card not linked)
            if (btn.disabled) return;
            const action = btn.dataset.action;
            const placeId = parseInt(btn.dataset.placeId, 10);
            const sessionId = btn.dataset.sessionId || null;
            // sync, extract, test - pass projectDir, placeId, and sessionId
            const dir = btn.dataset.dir;
            vscode.postMessage({ command: action, projectDir: dir, placeId: placeId, sessionId: sessionId });
          };
        });
      }

      // E2E toggle
      document.getElementById('e2eToggle').classList.toggle('on', s.e2eModeEnabled);

      // Rbxjson toggle (on = visible, off = hidden)
      document.getElementById('rbxjsonToggle').classList.toggle('on', !s.rbxjsonHidden);

      // Cat visibility (default to visible if not set)
      const catVisible = s.catVisible !== false;
      document.getElementById('zenCat').classList.toggle('hidden', !catVisible);
      document.getElementById('catSpacer').classList.toggle('hidden', !catVisible);
      document.body.classList.toggle('cat-hidden', !catVisible);
    }

    function shortenPath(p) {
      if (!p) return '';
      const parts = p.split('/');
      if (parts.length > 3) return '.../' + parts.slice(-2).join('/');
      return p;
    }

    function relTime(ts) {
      const s = Math.floor((Date.now() - ts) / 1000);
      if (s < 5) return 'now';
      if (s < 60) return s + 's';
      const m = Math.floor(s / 60);
      return m + 'm';
    }

    // Collapsible sections
    document.querySelectorAll('.section-header').forEach(header => {
      header.addEventListener('click', () => {
        const section = header.dataset.section;
        const content = document.getElementById(section + 'Content');
        if (content) {
          header.classList.toggle('collapsed');
          content.classList.toggle('visible');
        }
      });
    });

    document.getElementById('serverBtn').onclick = () => {
      vscode.postMessage({ command: state?.connectionStatus === 'connected' ? 'disconnect' : 'connect' });
    };
    document.getElementById('consoleBtn').onclick = () => vscode.postMessage({ command: 'openConsole' });
    document.getElementById('e2eBtn').onclick = () => vscode.postMessage({ command: 'toggleE2E' });
    document.getElementById('rbxjsonBtn').onclick = () => vscode.postMessage({ command: 'toggleRbxjson' });

    vscode.postMessage({ command: 'ready' });
  </script>
</body>
</html>`;
  }
}

function getNonce(): string {
  let text = '';
  const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
