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
    catMood: 'idle'
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
      padding: 12px;
      line-height: 1.5;
      /* Override VS Code theme for consistent look */
      --vscode-sideBar-background: var(--bg-base);
    }

    /* Result Toast */
    .toast {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px 12px;
      border-radius: var(--radius);
      margin-bottom: 12px;
      animation: slideIn 0.2s ease;
      font-size: 11px;
      font-weight: 500;
    }
    .toast.success { background: var(--success-soft); border: 1px solid var(--success); color: var(--success); }
    .toast.error { background: var(--error-soft); border: 1px solid var(--error); color: var(--error); }
    .toast .icon { width: 14px; height: 14px; flex-shrink: 0; }
    .toast-text { flex: 1; }
    .toast-time { opacity: 0.7; font-size: 10px; }

    @keyframes slideIn {
      from { opacity: 0; transform: translateY(-8px); }
      to { opacity: 1; transform: translateY(0); }
    }

    .spinner {
      width: 14px; height: 14px;
      border: 2px solid transparent;
      border-top-color: currentColor;
      border-radius: 50%;
      animation: spin 0.7s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }

    /* Section */
    .section { margin-bottom: 16px; }
    .section-title {
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 10px;
      font-weight: 600;
      color: var(--text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.5px;
      margin-bottom: 8px;
      padding: 0 2px;
    }
    .section-title .icon { width: 12px; height: 12px; opacity: 0.7; }
    .section-title .count {
      margin-left: auto;
      background: var(--bg-elevated);
      padding: 2px 6px;
      border-radius: 10px;
      font-size: 9px;
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

    /* Sync = Primary button (solid green) */
    .studio-btn.sync {
      background: var(--accent);
      border-color: var(--accent);
      color: var(--bg-base);
    }
    .studio-btn.sync:hover {
      background: var(--accent-hover);
      border-color: var(--accent-hover);
      color: var(--bg-base);
    }
    .studio-btn.sync .icon { opacity: 1; }

    /* Extract, Test = Secondary buttons (outlined) */
    .studio-btn.extract:hover { border-color: var(--border-light); color: var(--text-primary); }
    .studio-btn.test:hover { border-color: var(--border-light); color: var(--text-primary); }

    /* Link/Unlink buttons */
    .studio-btn.link { background: var(--success-soft); border-color: var(--success); color: var(--success); }
    .studio-btn.link:hover { background: var(--success); color: #fff; }
    .studio-btn.unlink { background: var(--warning-soft); border-color: var(--warning); color: var(--warning); }
    .studio-btn.unlink:hover { background: var(--warning); color: #fff; }

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
      text-align: center;
      padding: 24px 16px;
      color: var(--text-secondary);
    }
    .empty-state .icon {
      width: 40px; height: 40px;
      margin: 0 auto 12px;
      opacity: 0.4;
    }
    .empty-state h3 {
      font-size: 13px;
      font-weight: 600;
      color: var(--text-primary);
      margin-bottom: 4px;
    }
    .empty-state p {
      font-size: 11px;
      margin-bottom: 12px;
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

    /* Keyboard hint */
    .keyboard-hint {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
      padding: 8px;
      background: var(--bg-surface);
      border-radius: var(--radius-sm);
      font-size: 10px;
      color: var(--text-muted);
      margin-top: 8px;
    }
    .kbd {
      background: var(--bg-elevated);
      padding: 2px 5px;
      border-radius: 3px;
      font-family: var(--vscode-editor-font-family, monospace);
      font-size: 9px;
      border: 1px solid var(--border);
    }

    .hidden { display: none !important; }

    /* Zen Cat Mascot - Compact */
    .zen-cat-container {
      display: flex;
      align-items: center;
      gap: 8px;
      background: var(--bg-surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      padding: 6px 10px;
      margin-bottom: 8px;
      position: relative;
      overflow: hidden;
      cursor: pointer;
      user-select: none;
    }
    .zen-cat-container:hover {
      border-color: var(--border-light);
    }
    .zen-cat-container:active .zen-cat {
      transform: scale(0.95);
    }
    .zen-cat-container::before {
      content: '';
      position: absolute;
      top: 0; left: 0; right: 0;
      height: 2px;
      background: linear-gradient(90deg, #f472b6, #a78bfa, #60a5fa, #4ade80, #facc15, #f472b6);
      background-size: 200% 100%;
      animation: rainbow-slide 3s linear infinite;
    }
    @keyframes rainbow-slide {
      0% { background-position: 0% 0%; }
      100% { background-position: 200% 0%; }
    }
    .zen-cat {
      font-family: var(--vscode-editor-font-family, monospace);
      font-size: 9px;
      line-height: 1.1;
      white-space: pre;
      flex-shrink: 0;
      transition: color 0.3s ease;
    }
    .zen-cat.idle { color: #a78bfa; }
    .zen-cat.syncing { color: #60a5fa; animation: cat-bounce 0.5s ease infinite; }
    .zen-cat.success { color: #4ade80; }
    .zen-cat.error { color: #f87171; }
    @keyframes cat-bounce {
      0%, 100% { transform: translateY(0); }
      50% { transform: translateY(-1px); }
    }
    .zen-quote-feed {
      flex: 1;
      display: flex;
      align-items: center;
      min-width: 0;
    }
    .zen-quote {
      font-size: 10px;
      color: var(--text-muted);
      font-style: italic;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      opacity: 1;
      transition: opacity 0.5s ease;
    }
    .zen-quote.fade { opacity: 0; }
    .zen-cat-container:hover .zen-quote { color: var(--text-secondary); }

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

    /* Version Footer */
    .version-footer {
      text-align: center;
      font-size: 10px;
      color: var(--text-muted);
      margin-top: 16px;
      padding-top: 12px;
      border-top: 1px solid var(--border);
    }
  </style>
</head>
<body>
  <!-- Update Banner -->
  <div class="update-banner hidden" id="updateBanner">
    <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
    <span class="text"><strong>v<span id="updateVersion"></span></strong> available</span>
    <button class="dismiss" id="dismissUpdate">×</button>
  </div>

  <!-- Zen Cat Mascot -->
  <div class="zen-cat-container" id="zenCat">
    <div class="zen-cat idle" id="zenCatArt"></div>
    <div class="zen-quote-feed">
      <div class="zen-quote" id="zenQuote"></div>
    </div>
  </div>

  <!-- Toast -->
  <div class="toast hidden" id="toast">
    <svg class="icon success-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M20 6L9 17l-5-5"/></svg>
    <svg class="icon error-icon hidden" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><path d="M15 9l-6 6M9 9l6 6"/></svg>
    <span class="toast-text" id="toastText"></span>
    <span class="toast-time" id="toastTime"></span>
  </div>

  <!-- Studios Section -->
  <div class="section">
    <div class="section-title">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
      Studios
      <span class="count" id="studioCount">0</span>
    </div>
    <div id="studioList"></div>
    <div class="empty-state" id="emptyState">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/><path d="M9 10h6M12 7v6" opacity="0.5"/></svg>
      <h3 id="emptyTitle">No Studios Connected</h3>
      <p id="emptyDesc">Open Roblox Studio and install the RbxSync plugin</p>
    </div>
  </div>

  <!-- Server Section -->
  <div class="section">
    <div class="section-title">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><circle cx="6" cy="6" r="1" fill="currentColor"/><circle cx="6" cy="18" r="1" fill="currentColor"/></svg>
      Server
    </div>
    <div class="server-bar">
      <div class="server-status">
        <div class="status-dot" id="serverDot"></div>
        <span class="server-label" id="serverLabel">Stopped</span>
      </div>
      <button class="server-btn start" id="serverBtn">Start</button>
    </div>
  </div>

  <!-- Tools Section -->
  <div class="section">
    <div class="section-title">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.7 6.3a1 1 0 000 1.4l1.6 1.6a1 1 0 001.4 0l3.77-3.77a6 6 0 01-7.94 7.94l-6.91 6.91a2.12 2.12 0 01-3-3l6.91-6.91a6 6 0 017.94-7.94l-3.76 3.76z"/></svg>
      Tools
    </div>
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
      <span class="label">Toggle .rbxjson</span>
      <svg class="arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18l6-6-6-6"/></svg>
    </div>
  </div>

  <!-- Keyboard Hints -->
  <div class="keyboard-hint">
    <span><kbd class="kbd">⌘⌥S</kbd> Sync</span>
    <span><kbd class="kbd">⌘⌥E</kbd> Extract</span>
    <span><kbd class="kbd">⌘⌥T</kbd> Test</span>
  </div>

  <!-- Version -->
  <div class="version-footer">
    RbxSync v${this._version}
  </div>

  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let state = null;

    // Zen Cat ASCII Art for different moods (compact)
    const CAT_ART = {
      idle: \`/\\\\_/\\\\
(-.-)\`,
      syncing: \`/\\\\_/\\\\
(o.o)\`,
      success: \`/\\\\_/\\\\
(^.^)\`,
      error: \`/\\\\_/\\\\
(>.<)\`
    };

    // Zen wisdom quotes
    const ZEN_QUOTES = [
      "Just breathe",
      "Be here now.",
      "Be right here, right now.",
      "There is only Now.",
      "Breathe and let be.",
      "Pay attention.",
      "Your body is present; is your mind?",
      "Be a witness, not a judge.",
      "Attention!",
      "What am I?",
      "Om mani padme hum",
      "When hungry, eat.",
      "When tired, sleep.",
      "Only go straight — don't know.",
      "You must become completely crazy.",
      "Zen mind is before thinking.",
      "Always keep \\"Don't know\\" mind.",
      "When you are not thinking, all things are the same.",
      "You are the sky.",
      "YOLO",
      "Where is my mind right now?",
      "Who is breathing?",
      "Focus on your breath.",
      "Feelings come and go like clouds.",
      "Let go of thinking.",
      "Just be in this moment.",
      "Do one thing at a time.",
      "Life is available only in the present moment.",
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
      "Do not be concerned with the fruits of your action.",
      "Do not dwell in the past.",
      "Do not dream of the future.",
      "Wear gratitude like a cloak.",
      "Let the beauty of what you love be what you do.",
      "My religion is love.",
      "Listen to your heart.",
      "The quieter you become, the more you hear.",
      "You are not a drop in the ocean.",
      "Love is the bridge between you and everything.",
      "When you know how to listen, everybody is the guru.",
      "We're all just walking each other home.",
      "Every person you look at is a soul.",
      "When you are fully in the moment, this moment is all there is.",
      "All problems are illusions of the mind.",
      "If not now, when?",
      "You cannot be both unhappy and fully present.",
      "Get the inside right.",
      "Find the life underneath your life situation.",
      "Give your fullest attention.",
      "Gate, gate, paragate...",
      "Zen is keeping the mind before thinking.",
      "Who is the master of this body?",
      "You are already that which you seek.",
      "Let come what comes.",
      "Let go what goes.",
      "See what remains.",
      "When there is no \\"I\\" there is no karma.",
      "No one succeeds without effort.",
      "In the beginning, meditation is within your day.",
      "Each time for the first time.",
      "You need nothing more.",
      "I am detached.",
      "There is only light.",
      "Your thoughts come and go.",
      "Live in the moment, live in the breath.",
      "The words you speak become the house you live in.",
      "Nothing can bring you peace but yourself.",
      "Be the change you want to see in the world.",
      "You can't stop the waves, but you can learn to surf.",
      "Have you eaten your porridge?",
      "Yes, I have.",
      "Then you better wash your bowl.",
      "Express yourself as you are.",
      "When you sit, everything sits with you.",
      "What we call \\"I\\" is just a swinging door.",
      "Do not serve your thoughts tea.",
      "Whatever the moment contains, accept it.",
      "Establish yourself in the present moment.",
      "Breathe in deeply.",
      "Breathe out slowly.",
      "Feel what you feel now.",
      "Go through your day as if undercover.",
      "Put it all down.",
      "Let it all go.",
      "Pay attention to the present moment, on purpose.",
      "We're present now.",
      "Be kind whenever possible.",
      "Nothing else matters now.",
      "Become still and alert."
    ];

    // Initialize zen cat quote feed
    let quoteIndex = 0;
    let shuffledQuotes = [];

    function initZenCat() {
      const quoteEl = document.getElementById('zenQuote');
      if (!quoteEl) return;

      // Shuffle quotes for variety
      shuffledQuotes = [...ZEN_QUOTES].sort(() => Math.random() - 0.5);
      quoteEl.textContent = shuffledQuotes[0];

      // Swap quote every 10 seconds
      setInterval(() => {
        const quoteEl = document.getElementById('zenQuote');
        if (!quoteEl) return;

        // Fade out
        quoteEl.classList.add('fade');

        setTimeout(() => {
          // Change quote
          quoteIndex = (quoteIndex + 1) % shuffledQuotes.length;
          quoteEl.textContent = shuffledQuotes[quoteIndex];
          // Fade in
          quoteEl.classList.remove('fade');
        }, 500);
      }, 10000);

      // Update cat art
      updateCatMood('idle');
    }

    function updateCatMood(mood) {
      const catEl = document.getElementById('zenCatArt');
      if (!catEl) return;

      catEl.textContent = CAT_ART[mood] || CAT_ART.idle;
      catEl.className = 'zen-cat ' + mood;
    }

    // Cat click reaction
    function onCatClick() {
      const catEl = document.getElementById('zenCatArt');
      const quoteEl = document.getElementById('zenQuote');
      if (!catEl) return;

      // Show surprised face
      catEl.textContent = \`/\\\\_/\\\\
(O.O)\`;
      catEl.style.color = '#facc15';

      // Show new quote immediately
      if (quoteEl && shuffledQuotes.length > 0) {
        quoteIndex = (quoteIndex + 1) % shuffledQuotes.length;
        quoteEl.textContent = shuffledQuotes[quoteIndex];
      }

      // Return to normal after a moment
      setTimeout(() => {
        updateCatMood(state?.catMood || 'idle');
      }, 800);
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
      // Update zen cat mood
      updateCatMood(s.catMood || 'idle');

      // Toast
      const toast = document.getElementById('toast');
      if (s.lastResult) {
        toast.classList.remove('hidden');
        toast.className = 'toast ' + (s.lastResult.success ? 'success' : 'error');
        document.getElementById('toastText').textContent = s.lastResult.label;
        document.getElementById('toastTime').textContent = relTime(s.lastResult.time);
        document.querySelector('.success-icon').classList.toggle('hidden', !s.lastResult.success);
        document.querySelector('.error-icon').classList.toggle('hidden', s.lastResult.success);
      } else {
        toast.classList.add('hidden');
      }

      // Server
      const isOn = s.connectionStatus === 'connected';
      const isConnecting = s.connectionStatus === 'connecting';
      document.getElementById('serverDot').className = 'status-dot' + (isOn ? ' on' : isConnecting ? ' connecting' : '');
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
              <button class="studio-btn sync" data-action="sync" data-dir="\${place.project_dir}" data-place-id="\${place.place_id}" data-session-id="\${place.session_id || ''}">
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
                  <polyline points="17 8 12 3 7 8"/>
                  <line x1="12" y1="3" x2="12" y2="15"/>
                </svg>
                Sync
              </button>
              <button class="studio-btn extract" data-action="extract" data-dir="\${place.project_dir}" data-place-id="\${place.place_id}" data-session-id="\${place.session_id || ''}">
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
                  <polyline points="7 10 12 15 17 10"/>
                  <line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                Extract
              </button>
              <button class="studio-btn test" data-action="test" data-dir="\${place.project_dir}" data-place-id="\${place.place_id}" data-session-id="\${place.session_id || ''}">
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
