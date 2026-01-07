// TypeScript interfaces matching rbxsync-server responses

export interface HealthResponse {
  status: string;
  version: string;
  connected: boolean;
}

export interface ExtractStartRequest {
  project_dir: string;
  services?: string[];
}

export interface ExtractStartResponse {
  session_id: string;
  message: string;
}

export interface ExtractStatusResponse {
  status: 'pending' | 'extracting' | 'complete' | 'error';
  current_service?: string;
  instances_extracted: number;
  total_services: number;
  services_complete: number;
  error?: string;
}

export interface ExtractFinalizeRequest {
  session_id: string;
  project_dir: string;
}

export interface ExtractFinalizeResponse {
  success: boolean;
  files_written: number;
  message: string;
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
  errors: string[];
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
