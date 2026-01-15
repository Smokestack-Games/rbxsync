// TypeScript interfaces matching rbxsync-server responses

export interface HealthResponse {
  status: string;
  version: string;
  connected: boolean;
}

export interface ExtractStartRequest {
  project_dir: string;
  services?: string[];
  include_terrain?: boolean;
}

export interface ExtractStartResponse {
  session_id: string;
  message: string;
}

export interface ExtractStatusResponse {
  sessionId: string;
  chunksReceived: number;
  totalChunks: number;
  complete: boolean;
  error?: string;
}

export interface ExtractFinalizeRequest {
  session_id: string;
  project_dir: string;
}

export interface ExtractFinalizeResponse {
  success: boolean;
  filesWritten: number;
  scriptsWritten: number;
  totalInstances: number;
}

export interface SyncReadTreeRequest {
  project_dir: string;
}

export interface SyncReadTreeResponse {
  instances: InstanceData[];
  total_count: number;
}

export interface InstanceData {
  path: string;
  class_name: string;
  name: string;
  properties: Record<string, PropertyValue>;
  source?: string;
}

export interface PropertyValue {
  type: string;
  value: unknown;
}

export interface SyncBatchRequest {
  operations: SyncOperation[];
  project_dir?: string;  // For routing to specific Studio
}

export interface SyncOperation {
  op: 'create' | 'update' | 'delete';
  path: string;
  class_name?: string;
  name?: string;
  properties?: Record<string, PropertyValue>;
  source?: string;
}

export interface SyncBatchResponse {
  success: boolean;
  applied: number;
  skipped?: number;
  errors: string[];
}

export interface StudioPathsResponse {
  paths: string[];
}

export interface SyncReadTerrainRequest {
  project_dir: string;
}

export interface SyncReadTerrainResponse {
  success: boolean;
  terrain?: unknown;
  error?: string;
}

export interface GitStatusResponse {
  is_repo: boolean;
  branch?: string;
  staged: string[];
  modified: string[];
  untracked: string[];
  ahead?: number;
  behind?: number;
}

export interface GitLogEntry {
  hash: string;
  short_hash: string;
  message: string;
  author: string;
  date: string;
}

export interface GitLogResponse {
  commits: GitLogEntry[];
}

export interface GitCommitRequest {
  project_dir: string;
  message: string;
  files?: string[];
}

export interface GitCommitResponse {
  success: boolean;
  hash?: string;
  error?: string;
}

export interface ConnectionState {
  connected: boolean;
  serverVersion?: string;
  lastError?: string;
  place?: PlaceInfo;  // Connected Studio place (if any)
}

// Multi-workspace support
export interface PlaceInfo {
  place_id: number;
  place_name: string;
  project_dir: string;
  session_id?: string;  // Unique session ID for this Studio instance
}

export interface PlacesResponse {
  places: PlaceInfo[];
}

// Test Runner Types
export interface ConsoleMessage {
  message: string;
  type: string;  // "MessageOutput" | "MessageWarning" | "MessageError" | "MessageInfo"
  timestamp: number;
}

export interface TestStartResponse {
  success: boolean;
  message?: string;
}

export interface TestStatusResponse {
  inProgress: boolean;
  complete: boolean;
  error?: string;
  output: ConsoleMessage[];
  totalMessages: number;
}

export interface TestFinishResponse {
  success: boolean;
  duration?: number;
  output: ConsoleMessage[];
  totalMessages: number;
  error?: string;
}

// Generic command response wrapper from /sync/command endpoint
export interface CommandResponse<T> {
  success: boolean;
  data: T;
  error?: string;
}

// Path mismatch info returned when VS Code workspace doesn't match Studio project
export interface PathMismatch {
  vscode_path: string;
  studio_paths: string[];
  warning: string;
}

// Registration response that may include path mismatch
export interface RegisterWorkspaceResponse {
  success: boolean;
  message: string;
  path_mismatch?: PathMismatch;
}
