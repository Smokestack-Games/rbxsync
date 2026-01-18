import axios, { AxiosInstance, AxiosError } from 'axios';
import * as vscode from 'vscode';
import {
  HealthResponse,
  ExtractStartRequest,
  ExtractStartResponse,
  ExtractStatusResponse,
  ExtractFinalizeRequest,
  ExtractFinalizeResponse,
  SyncReadTreeRequest,
  SyncReadTreeResponse,
  SyncBatchRequest,
  SyncBatchResponse,
  SyncIncrementalRequest,
  SyncIncrementalResponse,
  DiffResponse,
  GitStatusResponse,
  GitLogResponse,
  GitCommitRequest,
  GitCommitResponse,
  ConnectionState,
  TestStartResponse,
  TestStatusResponse,
  TestFinishResponse,
  CommandResponse,
  PlaceInfo,
  PlacesResponse,
  RegisterWorkspaceResponse,
  PathMismatch
} from './types';

export class RbxSyncClient {
  private client: AxiosInstance;
  private _connectionState: ConnectionState = { connected: false };
  private _onConnectionChange = new vscode.EventEmitter<ConnectionState>();
  private _projectDir: string = '';
  private _onLinkedStudioChange = new vscode.EventEmitter<{ placeId: number | null; sessionId: string | null }>();

  public readonly onConnectionChange = this._onConnectionChange.event;
  public readonly onLinkedStudioChange = this._onLinkedStudioChange.event;

  constructor(port: number = 44755) {
    this.client = axios.create({
      baseURL: `http://127.0.0.1:${port}`,
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json'
      }
    });
  }

  get connectionState(): ConnectionState {
    return this._connectionState;
  }

  get projectDir(): string {
    return this._projectDir;
  }

  setProjectDir(dir: string): void {
    this._projectDir = dir;
  }

  /**
   * Get the linked Studio place ID from workspace settings
   */
  getLinkedStudioId(): number | null {
    const config = vscode.workspace.getConfiguration('rbxsync');
    const linkedId = config.get<number | null>('linkedStudioId');
    return linkedId ?? null;
  }

  /**
   * Get the linked Studio session ID from workspace settings (for unpublished places)
   */
  getLinkedStudioSessionId(): string | null {
    const config = vscode.workspace.getConfiguration('rbxsync');
    const sessionId = config.get<string | null>('linkedStudioSessionId');
    return sessionId ?? null;
  }

  /**
   * Set the linked Studio in workspace settings
   */
  async setLinkedStudio(placeId: number | null, sessionId: string | null = null): Promise<void> {
    const config = vscode.workspace.getConfiguration('rbxsync');
    await config.update('linkedStudioId', placeId, vscode.ConfigurationTarget.Workspace);
    await config.update('linkedStudioSessionId', sessionId, vscode.ConfigurationTarget.Workspace);
    this._onLinkedStudioChange.fire({ placeId, sessionId });
  }

  /**
   * Check if a specific place is the linked studio for this workspace
   */
  isLinkedStudio(placeId: number, sessionId?: string): boolean {
    const linkedId = this.getLinkedStudioId();
    const linkedSession = this.getLinkedStudioSessionId();

    // For unpublished places (placeId=0), use sessionId
    if (placeId === 0 && sessionId) {
      return linkedSession === sessionId;
    }

    // For published places, use placeId
    return linkedId === placeId;
  }

  /**
   * Get the linked place info from connected places
   */
  async getLinkedPlace(): Promise<PlaceInfo | undefined> {
    const places = await this.getConnectedPlaces();
    const linkedId = this.getLinkedStudioId();
    const linkedSession = this.getLinkedStudioSessionId();

    // For unpublished places, match by session
    if (linkedSession) {
      const bySession = places.find(p => p.session_id === linkedSession);
      if (bySession) return bySession;
    }

    // For published places, match by placeId
    if (linkedId !== null && linkedId > 0) {
      return places.find(p => p.place_id === linkedId);
    }

    return undefined;
  }

  /**
   * Show quick pick to select a Studio to link
   */
  async promptLinkToStudio(): Promise<boolean> {
    const places = await this.getConnectedPlaces();

    if (places.length === 0) {
      vscode.window.showWarningMessage('No Studio instances connected. Open Roblox Studio with the RbxSync plugin installed.');
      return false;
    }

    interface PlaceQuickPickItem extends vscode.QuickPickItem {
      place: PlaceInfo;
    }

    const linkedId = this.getLinkedStudioId();
    const linkedSession = this.getLinkedStudioSessionId();

    const items: PlaceQuickPickItem[] = places.map(place => {
      const isCurrentlyLinked = this.isLinkedStudio(place.place_id, place.session_id);
      return {
        label: `$(${isCurrentlyLinked ? 'check' : 'circle-outline'}) ${place.place_name || 'Unnamed Place'}`,
        description: place.place_id > 0 ? `ID: ${place.place_id}` : 'Unpublished',
        detail: isCurrentlyLinked ? '$(link) Currently linked to this workspace' : undefined,
        place
      };
    });

    // Add unlink option if currently linked
    if (linkedId !== null || linkedSession !== null) {
      items.push({
        label: '$(debug-disconnect) Unlink from Studio',
        description: 'Remove the link to allow seeing all Studios',
        place: null as any
      });
    }

    const selected = await vscode.window.showQuickPick(items, {
      placeHolder: 'Select a Studio instance to link to this workspace',
      title: 'Link to Studio'
    });

    if (!selected) {
      return false;
    }

    // Handle unlink
    if (!selected.place) {
      await this.setLinkedStudio(null, null);
      vscode.window.showInformationMessage('Unlinked from Studio - all connected Studios are now visible');
      return true;
    }

    const place = selected.place;
    const sessionId = place.place_id === 0 ? place.session_id : null;
    await this.setLinkedStudio(place.place_id > 0 ? place.place_id : 0, sessionId || null);

    // Also notify the server about the link
    if (this._projectDir) {
      await this.linkStudio(place.place_id);
    }

    vscode.window.showInformationMessage(`Linked to "${place.place_name}" - this Studio is now the default for this workspace`);
    return true;
  }

  private updateConnectionState(state: ConnectionState): void {
    this._connectionState = state;
    this._onConnectionChange.fire(state);
  }

  // Health & Connection
  async checkHealth(): Promise<HealthResponse | null> {
    try {
      const response = await this.client.get<HealthResponse>('/health');

      // Fetch connected place for this project directory
      let place: PlaceInfo | undefined;
      if (this._projectDir) {
        place = await this.getConnectedPlace();
      }

      this.updateConnectionState({
        connected: true,
        serverVersion: response.data.version,
        place
      });
      return response.data;
    } catch (error) {
      const errorMessage = this.getErrorMessage(error);
      this.updateConnectionState({
        connected: false,
        lastError: errorMessage
      });
      return null;
    }
  }

  async connect(): Promise<boolean> {
    const health = await this.checkHealth();
    if (health && this._projectDir) {
      // Register workspace with server
      await this.registerWorkspace(this._projectDir);
    }
    return health !== null;
  }

  // Register VS Code workspace with server
  async registerWorkspace(workspaceDir: string): Promise<PathMismatch | null> {
    try {
      const response = await this.client.post<RegisterWorkspaceResponse>('/rbxsync/register-vscode', {
        workspace_dir: workspaceDir
      });

      // Check for path mismatch and show warning with action button
      if (response.data.path_mismatch) {
        const mismatch = response.data.path_mismatch;
        const studioPath = mismatch.studio_paths[0] || 'unknown';

        const action = await vscode.window.showWarningMessage(
          `Path Mismatch: VS Code is open at "${workspaceDir}" but Studio project is at "${studioPath}". Extracted files will go to the Studio path!`,
          'Update Studio Path',
          'Ignore'
        );

        if (action === 'Update Studio Path') {
          // Update the Studio plugin to use the VS Code workspace path
          await this.updateStudioProjectPath(workspaceDir);
        }

        return mismatch;
      }

      return null;
    } catch (error) {
      return null;
    }
  }

  // Update the project path in the connected Studio plugin
  async updateStudioProjectPath(newPath: string): Promise<boolean> {
    try {
      const response = await this.client.post('/rbxsync/update-project-path', {
        project_dir: newPath
      });
      if (response.data.success) {
        vscode.window.showInformationMessage(`Studio project path updated to: ${newPath}`);
        return true;
      }
      return false;
    } catch (error) {
      vscode.window.showErrorMessage('Failed to update Studio project path. Make sure Studio is connected.');
      return false;
    }
  }

  // Get all connected Studio places
  async getConnectedPlaces(): Promise<PlaceInfo[]> {
    try {
      const response = await this.client.get<PlacesResponse>('/rbxsync/places');
      return response.data.places || [];
    } catch (error) {
      return [];
    }
  }

  // Get the place connected to this workspace's project directory
  async getConnectedPlace(): Promise<PlaceInfo | undefined> {
    if (!this._projectDir) return undefined;

    const places = await this.getConnectedPlaces();
    return places.find(p => p.project_dir === this._projectDir);
  }

  // Get current operation status from server (RBXSYNC-77)
  async getOperationStatus(projectDir?: string): Promise<import('./types').OperationInfo | null> {
    try {
      const params = projectDir ? `?projectDir=${encodeURIComponent(projectDir)}` : '';
      const response = await this.client.get<import('./types').OperationStatusResponse>(`/rbxsync/status${params}`);
      return response.data.operation || null;
    } catch (error) {
      return null;
    }
  }

  // Link a specific Studio place to this workspace
  async linkStudio(placeId: number): Promise<boolean> {
    if (!this._projectDir) {
      vscode.window.showErrorMessage('No workspace folder open');
      return false;
    }

    try {
      const response = await this.client.post('/rbxsync/link-studio', {
        place_id: placeId,
        new_project_dir: this._projectDir
      });

      if (response.data.success) {
        vscode.window.showInformationMessage(`Linked ${response.data.place_name} to this workspace`);
        // Trigger connection change to refresh places
        const places = await this.getConnectedPlaces();
        const place = places.find(p => p.project_dir === this._projectDir);
        this.updateConnectionState({
          connected: true,
          serverVersion: this._connectionState.serverVersion,
          place
        });
        return true;
      } else {
        vscode.window.showErrorMessage(response.data.error || 'Failed to link studio');
        return false;
      }
    } catch (error) {
      vscode.window.showErrorMessage('Failed to link studio');
      return false;
    }
  }

  // Unlink a Studio place from this workspace
  async unlinkStudio(placeId: number): Promise<boolean> {
    try {
      const response = await this.client.post('/rbxsync/unlink-studio', {
        place_id: placeId
      });

      if (response.data.success) {
        vscode.window.showInformationMessage(`Unlinked ${response.data.place_name} from this workspace`);
        // Trigger connection change to refresh places
        const places = await this.getConnectedPlaces();
        const place = places.find(p => p.project_dir === this._projectDir);
        this.updateConnectionState({
          connected: true,
          serverVersion: this._connectionState.serverVersion,
          place
        });
        return true;
      } else {
        vscode.window.showErrorMessage(response.data.error || 'Failed to unlink studio');
        return false;
      }
    } catch (error) {
      vscode.window.showErrorMessage('Failed to unlink studio');
      return false;
    }
  }

  // Undo last extraction by restoring from backup
  async undoExtract(projectDir?: string): Promise<boolean> {
    try {
      const dir = projectDir || this._projectDir;
      const response = await this.client.post('/rbxsync/undo-extract', {
        project_dir: dir
      });

      if (response.data.success) {
        vscode.window.showInformationMessage('Extraction undone - src restored from backup');
        return true;
      } else {
        vscode.window.showErrorMessage(response.data.error || 'Failed to undo extraction');
        return false;
      }
    } catch (error) {
      vscode.window.showErrorMessage('Failed to undo extraction - no backup found');
      return false;
    }
  }

  // Extraction
  async startExtraction(projectDir: string, services?: string[], includeTerrain: boolean = true): Promise<ExtractStartResponse | null> {
    try {
      const request: ExtractStartRequest = {
        project_dir: projectDir,
        services,
        include_terrain: includeTerrain
      };
      const response = await this.client.post<ExtractStartResponse>('/extract/start', request);
      return response.data;
    } catch (error) {
      this.handleError('Start extraction', error);
      return null;
    }
  }

  async getExtractionStatus(): Promise<ExtractStatusResponse | null> {
    try {
      const response = await this.client.get<ExtractStatusResponse>('/extract/status');
      return response.data;
    } catch (error) {
      this.handleError('Get extraction status', error);
      return null;
    }
  }

  async finalizeExtraction(sessionId: string, projectDir: string): Promise<ExtractFinalizeResponse | null> {
    try {
      const request: ExtractFinalizeRequest = {
        session_id: sessionId,
        project_dir: projectDir
      };
      const response = await this.client.post<ExtractFinalizeResponse>('/extract/finalize', request);
      return response.data;
    } catch (error) {
      this.handleError('Finalize extraction', error);
      return null;
    }
  }

  // Sync
  async readTree(projectDir: string): Promise<SyncReadTreeResponse | null> {
    try {
      const request: SyncReadTreeRequest = { project_dir: projectDir };
      const response = await this.client.post<SyncReadTreeResponse>('/sync/read-tree', request);
      return response.data;
    } catch (error) {
      this.handleError('Read tree', error);
      return null;
    }
  }

  async syncIncremental(projectDir: string, markSynced: boolean = false): Promise<SyncIncrementalResponse | null> {
    try {
      const request: SyncIncrementalRequest = { project_dir: projectDir, mark_synced: markSynced };
      const response = await this.client.post<SyncIncrementalResponse>('/sync/incremental', request);
      return response.data;
    } catch (error) {
      this.handleError('Incremental sync', error);
      return null;
    }
  }

  async getDiff(projectDir: string): Promise<DiffResponse | null> {
    try {
      const response = await this.client.post<DiffResponse>('/diff', { project_dir: projectDir });
      return response.data;
    } catch (error) {
      this.handleError('Get diff', error);
      return null;
    }
  }

  async syncBatch(operations: SyncBatchRequest['operations'], projectDir?: string): Promise<SyncBatchResponse | null> {
    try {
      const request: SyncBatchRequest = { operations, project_dir: projectDir };
      const response = await this.client.post<SyncBatchResponse>('/sync/batch', request);
      return response.data;
    } catch (error) {
      this.handleError('Sync batch', error);
      return null;
    }
  }

  async getStudioPaths(): Promise<string[] | null> {
    try {
      const response = await this.client.post<{ paths: Array<{ path: string; className: string; name: string }> }>('/studio/paths', {});
      // Extract just the path strings from the response objects
      return response.data?.paths?.map(p => p.path) || [];
    } catch (error) {
      this.handleError('Get studio paths', error);
      return null;
    }
  }

  async readTerrain(projectDir: string): Promise<{ terrain: unknown } | null> {
    try {
      const response = await this.client.post<{ success: boolean; terrain?: unknown }>('/sync/read-terrain', { project_dir: projectDir });
      if (response.data?.success && response.data?.terrain) {
        return { terrain: response.data.terrain };
      }
      return null;
    } catch (error) {
      // Terrain might not exist, not an error
      return null;
    }
  }

  // Git
  async getGitStatus(projectDir: string): Promise<GitStatusResponse | null> {
    try {
      const response = await this.client.post<GitStatusResponse>('/git/status', { project_dir: projectDir });
      return response.data;
    } catch (error) {
      this.handleError('Get git status', error);
      return null;
    }
  }

  async getGitLog(projectDir: string, limit: number = 20): Promise<GitLogResponse | null> {
    try {
      const response = await this.client.post<GitLogResponse>('/git/log', {
        project_dir: projectDir,
        limit
      });
      return response.data;
    } catch (error) {
      this.handleError('Get git log', error);
      return null;
    }
  }

  async gitCommit(projectDir: string, message: string, files?: string[]): Promise<GitCommitResponse | null> {
    try {
      const request: GitCommitRequest = {
        project_dir: projectDir,
        message,
        files
      };
      const response = await this.client.post<GitCommitResponse>('/git/commit', request);
      return response.data;
    } catch (error) {
      this.handleError('Git commit', error);
      return null;
    }
  }

  // Test Runner (uses /sync/command endpoint)
  async runTest(duration?: number, mode?: string, projectDir?: string): Promise<TestStartResponse | null> {
    try {
      const payload: Record<string, unknown> = {};
      if (duration !== undefined) {
        payload.duration = duration;
      }
      if (mode !== undefined) {
        payload.mode = mode;
      }
      const response = await this.client.post<CommandResponse<TestStartResponse>>('/sync/command', {
        command: 'test:run',
        payload,
        project_dir: projectDir
      });
      return response.data.data;
    } catch (error) {
      this.handleError('Run test', error);
      return null;
    }
  }

  async getTestStatus(projectDir?: string): Promise<TestStatusResponse | null> {
    try {
      const response = await this.client.post<CommandResponse<TestStatusResponse>>('/sync/command', {
        command: 'test:status',
        payload: {},
        project_dir: projectDir
      });
      return response.data.data;
    } catch (error) {
      this.handleError('Get test status', error);
      return null;
    }
  }

  async finishTest(projectDir?: string): Promise<TestFinishResponse | null> {
    try {
      const response = await this.client.post<CommandResponse<TestFinishResponse>>('/sync/command', {
        command: 'test:finish',
        payload: {},
        project_dir: projectDir
      });
      return response.data.data;
    } catch (error) {
      this.handleError('Finish test', error);
      return null;
    }
  }

  // Helpers
  private getErrorMessage(error: unknown): string {
    if (axios.isAxiosError(error)) {
      const axiosError = error as AxiosError;
      if (axiosError.code === 'ECONNREFUSED') {
        return 'Server not running. Start the rbxsync server first.';
      }
      if (axiosError.response) {
        const data = axiosError.response.data as { error?: string };
        return data.error || `Server error: ${axiosError.response.status}`;
      }
      return axiosError.message;
    }
    return String(error);
  }

  private handleError(operation: string, error: unknown): void {
    const message = this.getErrorMessage(error);
    console.error(`RbxSync: ${operation} failed: ${message}`);
  }

  dispose(): void {
    this._onConnectionChange.dispose();
    this._onLinkedStudioChange.dispose();
  }
}
