import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

const TRASH_FOLDER = '.rbxsync-trash';
const TRASH_MANIFEST = 'manifest.json';
const TRASH_RETENTION_DAYS = 7;

/** Directories that should never be copied during recursive operations */
const SKIP_DIRS = new Set(['.rbxsync-trash', '.rbxsync-backup', '.rbxsync', '.git', 'node_modules']);

interface TrashEntry {
  originalPath: string;
  trashPath: string;
  deletedAt: number;
  isDirectory: boolean;
}

interface TrashManifest {
  entries: TrashEntry[];
}

/**
 * Get the trash directory for a workspace
 */
function getTrashDir(workspaceFolder: vscode.WorkspaceFolder): string {
  return path.join(workspaceFolder.uri.fsPath, TRASH_FOLDER);
}

/**
 * Get the manifest path for a workspace
 */
function getManifestPath(workspaceFolder: vscode.WorkspaceFolder): string {
  return path.join(getTrashDir(workspaceFolder), TRASH_MANIFEST);
}

/**
 * Load the trash manifest
 */
function loadManifest(workspaceFolder: vscode.WorkspaceFolder): TrashManifest {
  const manifestPath = getManifestPath(workspaceFolder);
  try {
    if (fs.existsSync(manifestPath)) {
      const content = fs.readFileSync(manifestPath, 'utf-8');
      return JSON.parse(content);
    }
  } catch (e) {
    console.error('Failed to load trash manifest:', e);
  }
  return { entries: [] };
}

/**
 * Save the trash manifest
 */
function saveManifest(workspaceFolder: vscode.WorkspaceFolder, manifest: TrashManifest): void {
  const trashDir = getTrashDir(workspaceFolder);
  const manifestPath = getManifestPath(workspaceFolder);

  if (!fs.existsSync(trashDir)) {
    fs.mkdirSync(trashDir, { recursive: true });
  }

  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
}

/**
 * Copy a file or directory to trash
 */
function copyToTrash(
  sourcePath: string,
  workspaceFolder: vscode.WorkspaceFolder
): TrashEntry | null {
  const trashDir = getTrashDir(workspaceFolder);
  const timestamp = Date.now();
  const relativePath = path.relative(workspaceFolder.uri.fsPath, sourcePath);
  const trashPath = path.join(trashDir, `${timestamp}_${relativePath.replace(/[/\\]/g, '_')}`);

  try {
    if (!fs.existsSync(trashDir)) {
      fs.mkdirSync(trashDir, { recursive: true });
    }

    const stat = fs.statSync(sourcePath);
    const isDirectory = stat.isDirectory();

    if (isDirectory) {
      // Recursively copy directory
      copyDirSync(sourcePath, trashPath);
    } else {
      // Copy file
      fs.copyFileSync(sourcePath, trashPath);
    }

    return {
      originalPath: sourcePath,
      trashPath,
      deletedAt: timestamp,
      isDirectory
    };
  } catch (e) {
    console.error('Failed to copy to trash:', e);
    return null;
  }
}

/**
 * Recursively copy a directory, skipping system directories and
 * preventing circular copies (dest inside src).
 */
function copyDirSync(src: string, dest: string): void {
  const resolvedSrc = path.resolve(src);
  const resolvedDest = path.resolve(dest);

  // Prevent circular copy: if dest is inside src, this would recurse infinitely
  if (resolvedDest.startsWith(resolvedSrc + path.sep)) {
    console.warn(`[RbxSync Trash] Skipping circular copy: ${dest} is inside ${src}`);
    return;
  }

  fs.mkdirSync(dest, { recursive: true });

  const entries = fs.readdirSync(src, { withFileTypes: true });
  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      copyDirSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

/**
 * Recursively copy a directory (for restore), with circular copy protection.
 */
function restoreDirSync(src: string, dest: string): void {
  const resolvedSrc = path.resolve(src);
  const resolvedDest = path.resolve(dest);

  if (resolvedDest.startsWith(resolvedSrc + path.sep)) {
    console.warn(`[RbxSync Trash] Skipping circular restore: ${dest} is inside ${src}`);
    return;
  }

  fs.mkdirSync(dest, { recursive: true });

  const entries = fs.readdirSync(src, { withFileTypes: true });
  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      restoreDirSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

/**
 * Clean up old trash entries (older than TRASH_RETENTION_DAYS)
 */
function cleanupOldTrash(workspaceFolder: vscode.WorkspaceFolder): void {
  const manifest = loadManifest(workspaceFolder);
  const cutoff = Date.now() - (TRASH_RETENTION_DAYS * 24 * 60 * 60 * 1000);

  const toKeep: TrashEntry[] = [];
  const toDelete: TrashEntry[] = [];

  for (const entry of manifest.entries) {
    if (entry.deletedAt < cutoff) {
      toDelete.push(entry);
    } else {
      toKeep.push(entry);
    }
  }

  // Delete old entries
  for (const entry of toDelete) {
    try {
      if (fs.existsSync(entry.trashPath)) {
        if (entry.isDirectory) {
          fs.rmSync(entry.trashPath, { recursive: true });
        } else {
          fs.unlinkSync(entry.trashPath);
        }
      }
    } catch (e) {
      console.error('Failed to cleanup trash entry:', e);
    }
  }

  // Update manifest
  manifest.entries = toKeep;
  saveManifest(workspaceFolder, manifest);
}

/**
 * Initialize the trash system
 * Listens for file deletions and backs them up
 */
export function initTrashSystem(context: vscode.ExtensionContext): void {
  // Listen for file deletions BEFORE they happen
  const deleteWatcher = vscode.workspace.onWillDeleteFiles(async (event) => {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders?.length) return;

    for (const deletion of event.files) {
      const filePath = deletion.fsPath;

      // Only backup files in src/ directories (RbxSync managed files)
      if (!filePath.includes(`${path.sep}src${path.sep}`)) continue;

      // Find the workspace folder for this file
      const workspaceFolder = workspaceFolders.find(wf =>
        filePath.startsWith(wf.uri.fsPath)
      );

      if (!workspaceFolder) continue;

      // Check if this is a directory (folder deletion)
      try {
        const stat = fs.statSync(filePath);
        if (stat.isDirectory()) {
          // Only backup directories (folders) - single file deletions are less critical
          const entry = copyToTrash(filePath, workspaceFolder);
          if (entry) {
            const manifest = loadManifest(workspaceFolder);
            manifest.entries.push(entry);
            saveManifest(workspaceFolder, manifest);
            console.log(`[RbxSync Trash] Backed up: ${filePath}`);
          }
        }
      } catch (e) {
        // File might not exist anymore
      }
    }
  });

  context.subscriptions.push(deleteWatcher);

  // Run cleanup on activation
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (workspaceFolders?.length) {
    for (const wf of workspaceFolders) {
      cleanupOldTrash(wf);
    }
  }
}

/**
 * Show a picker to recover deleted folders
 */
export async function recoverDeletedFolder(): Promise<void> {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders?.length) {
    vscode.window.showErrorMessage('No workspace folder open');
    return;
  }

  // Collect entries from all workspace folders
  const allEntries: { entry: TrashEntry; workspaceFolder: vscode.WorkspaceFolder }[] = [];

  for (const wf of workspaceFolders) {
    const manifest = loadManifest(wf);
    for (const entry of manifest.entries) {
      // Only show entries that still exist in trash
      if (fs.existsSync(entry.trashPath)) {
        allEntries.push({ entry, workspaceFolder: wf });
      }
    }
  }

  if (allEntries.length === 0) {
    vscode.window.showInformationMessage('No deleted folders to recover');
    return;
  }

  // Sort by deletion time (most recent first)
  allEntries.sort((a, b) => b.entry.deletedAt - a.entry.deletedAt);

  // Create quick pick items
  const items = allEntries.map(({ entry, workspaceFolder }) => {
    const relativePath = path.relative(workspaceFolder.uri.fsPath, entry.originalPath);
    const deletedDate = new Date(entry.deletedAt);
    const timeAgo = getTimeAgo(entry.deletedAt);

    return {
      label: relativePath,
      description: `Deleted ${timeAgo}`,
      detail: `Full path: ${entry.originalPath}`,
      entry,
      workspaceFolder
    };
  });

  const selected = await vscode.window.showQuickPick(items, {
    placeHolder: 'Select a folder to recover',
    matchOnDescription: true,
    matchOnDetail: true
  });

  if (!selected) return;

  // Restore the folder
  try {
    const { entry, workspaceFolder } = selected;

    // Check if original path already exists
    if (fs.existsSync(entry.originalPath)) {
      const overwrite = await vscode.window.showWarningMessage(
        `${path.basename(entry.originalPath)} already exists. Overwrite?`,
        'Overwrite',
        'Cancel'
      );

      if (overwrite !== 'Overwrite') return;

      // Remove existing
      fs.rmSync(entry.originalPath, { recursive: true });
    }

    // Restore from trash
    if (entry.isDirectory) {
      restoreDirSync(entry.trashPath, entry.originalPath);
    } else {
      const parentDir = path.dirname(entry.originalPath);
      if (!fs.existsSync(parentDir)) {
        fs.mkdirSync(parentDir, { recursive: true });
      }
      fs.copyFileSync(entry.trashPath, entry.originalPath);
    }

    // Remove from trash
    fs.rmSync(entry.trashPath, { recursive: true });

    // Update manifest
    const manifest = loadManifest(workspaceFolder);
    manifest.entries = manifest.entries.filter(e => e.trashPath !== entry.trashPath);
    saveManifest(workspaceFolder, manifest);

    vscode.window.showInformationMessage(`Recovered: ${path.basename(entry.originalPath)}`);
  } catch (e) {
    vscode.window.showErrorMessage(`Failed to recover: ${e}`);
  }
}

/**
 * Get human-readable time ago string
 */
function getTimeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);

  if (seconds < 60) return 'just now';
  if (seconds < 3600) return `${Math.floor(seconds / 60)} minutes ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} hours ago`;
  return `${Math.floor(seconds / 86400)} days ago`;
}
