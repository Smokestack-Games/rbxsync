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

  private state: SidebarState = {
    connectionStatus: 'disconnected',
    places: [],
    currentProjectDir: '',
    currentOperation: null,
    lastResult: null,
    e2eModeEnabled: false,
    serverRunning: false,
    studioOperations: {}
  };

  constructor(extensionUri: vscode.Uri) {
    this._extensionUri = extensionUri;
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

  // Studio operation tracking - keyed by sessionId or placeId
  public startStudioOperation(placeId: number, type: 'sync' | 'extract' | 'test', sessionId?: string | null): void {
    const studioKey = getStudioKey({ place_id: placeId, session_id: sessionId || undefined });
    this.state.studioOperations[studioKey] = {
      type,
      status: 'running',
      message: type === 'sync' ? 'Syncing...' : type === 'extract' ? 'Extracting...' : 'Testing...',
      startTime: Date.now()
    };
    this._updateWebview();
  }

  public completeStudioOperation(placeId: number, success: boolean, message: string, sessionId?: string | null): void {
    const studioKey = getStudioKey({ place_id: placeId, session_id: sessionId || undefined });
    const op = this.state.studioOperations[studioKey];
    if (op) {
      op.status = success ? 'success' : 'error';
      op.message = message;
      op.endTime = Date.now();
      this._updateWebview();

      // Clear after 30 seconds
      setTimeout(() => {
        if (this.state.studioOperations[studioKey] === op) {
          delete this.state.studioOperations[studioKey];
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
      --accent: #e2231a;
      --accent-soft: rgba(226, 35, 26, 0.15);
      --success: #10b981;
      --success-soft: rgba(16, 185, 129, 0.15);
      --warning: #f59e0b;
      --warning-soft: rgba(245, 158, 11, 0.15);
      --error: #ef4444;
      --error-soft: rgba(239, 68, 68, 0.15);
      --blue: #3b82f6;
      --blue-soft: rgba(59, 130, 246, 0.15);
      --purple: #8b5cf6;
      --purple-soft: rgba(139, 92, 246, 0.15);

      --bg-base: var(--vscode-sideBar-background, #1e1e1e);
      --bg-surface: var(--vscode-input-background, #252526);
      --bg-elevated: var(--vscode-dropdown-background, #2d2d30);
      --bg-hover: var(--vscode-list-hoverBackground, #2a2d2e);

      --text-primary: var(--vscode-foreground, #cccccc);
      --text-secondary: var(--vscode-descriptionForeground, #8b8b8b);
      --text-muted: var(--vscode-disabledForeground, #5a5a5a);

      --border: rgba(255, 255, 255, 0.08);
      --border-focus: rgba(255, 255, 255, 0.15);

      --radius: 8px;
      --radius-sm: 6px;
    }

    * { margin: 0; padding: 0; box-sizing: border-box; }

    body {
      font-family: var(--vscode-font-family, system-ui, sans-serif);
      font-size: 12px;
      color: var(--text-primary);
      background: var(--bg-base);
      padding: 12px;
      line-height: 1.5;
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
      background: var(--bg-elevated);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      color: var(--text-primary);
      font-family: inherit;
      font-size: 10px;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.15s;
    }
    .studio-btn:hover { background: var(--bg-hover); border-color: var(--border-focus); }
    .studio-btn .icon { width: 12px; height: 12px; }
    .studio-btn.sync:hover { border-color: var(--blue); color: var(--blue); }
    .studio-btn.extract:hover { border-color: var(--purple); color: var(--purple); }
    .studio-btn.test:hover { border-color: var(--success); color: var(--success); }
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
    .quick-row .toggle.on { background: var(--success); }
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
  </style>
</head>
<body>
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

  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let state = null;

    window.addEventListener('message', e => {
      if (e.data.type === 'stateUpdate') {
        state = e.data.state;
        render(state);
      }
    });

    function render(s) {
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
