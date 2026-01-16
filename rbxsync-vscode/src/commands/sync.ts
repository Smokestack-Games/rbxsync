import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { StatusBarManager } from '../views/statusBar';

export async function syncCommand(
  client: RbxSyncClient,
  statusBar: StatusBarManager,
  targetProjectDir?: string
): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('Not connected. Is Studio running?');
    return;
  }

  // Use provided projectDir or fall back to workspace
  let projectDir = targetProjectDir;
  if (!projectDir) {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders?.length) {
      vscode.window.showErrorMessage('Open a folder first.');
      return;
    }
    projectDir = workspaceFolders[0].uri.fsPath;
  }

  const config = vscode.workspace.getConfiguration('rbxsync');
  const deleteOrphans = config.get<boolean>('deleteOrphans') ?? true;
  const useDiffSync = config.get<boolean>('useDiffSync') ?? true;

  const operations: Array<{ type: string; path: string; data?: unknown }> = [];

  // Build set of file paths for orphan detection
  let filePaths: Set<string> | null = null;

  if (useDiffSync) {
    // Use diff-based sync: only sync changed files + creates + deletes
    console.log('[RbxSync] Using diff-based sync');

    // Get diff to find creates/deletes
    const diff = await client.getDiff(projectDir);
    if (!diff) {
      console.warn('[RbxSync] Failed to get diff, falling back to full sync');
    } else {
      console.log('[RbxSync] Diff: added=%d, removed=%d, common=%d',
        diff.added.length, diff.removed.length, diff.common);

      // Get incrementally changed files (modified since last sync)
      const incrementalResult = await client.syncIncremental(projectDir);
      if (incrementalResult) {
        console.log('[RbxSync] Incremental: %d changed files (full_sync=%s)',
          incrementalResult.count, incrementalResult.full_sync);

        // Add update operations for changed files
        for (const inst of incrementalResult.instances) {
          if (inst.path) {
            operations.push({
              type: 'update',
              path: inst.path,
              data: inst
            });
          }
        }
      }

      // If diff found items that need to be created (in files but not Studio),
      // we need to send those too (they may not be in incremental if files aren't new)
      if (diff.added.length > 0) {
        // Need full tree to get instance data for new items
        const treeResult = await client.readTree(projectDir);
        if (treeResult) {
          const addedPaths = new Set(diff.added.map(e => e.path));
          filePaths = new Set<string>();
          for (const inst of treeResult.instances) {
            if (inst.path) {
              filePaths.add(inst.path);
              // Only add operations for items that need to be created
              // or aren't already in our operations list
              if (addedPaths.has(inst.path) &&
                  !operations.some(op => op.path === inst.path)) {
                operations.push({
                  type: 'update',
                  path: inst.path,
                  data: inst
                });
              }
            }
          }
        }
      }

      // Handle orphan deletion using diff results
      if (deleteOrphans && diff.removed.length > 0) {
        const skipClasses = new Set([
          'Terrain', 'Camera', 'Workspace', 'ReplicatedStorage', 'ReplicatedFirst',
          'ServerScriptService', 'ServerStorage', 'StarterGui', 'StarterPack',
          'StarterPlayer', 'Lighting', 'SoundService', 'Teams', 'Chat',
          'LocalizationService', 'TestService', 'Players', 'RunService'
        ]);
        const skipNames = new Set(['SpawnLocation', 'Camera']);

        for (const entry of diff.removed) {
          const pathParts = entry.path.split('/');
          const instanceName = pathParts[pathParts.length - 1];

          // Skip services and special instances
          if (skipClasses.has(instanceName) || skipNames.has(instanceName)) continue;
          if (skipClasses.has(entry.className)) continue;

          // Skip the service itself (top level)
          if (pathParts.length === 1) continue;

          operations.push({
            type: 'delete',
            path: entry.path
          });
          console.log('[RbxSync] Will delete orphan:', entry.path);
        }
      }
    }
  }

  // Fall back to full sync if diff-based didn't produce operations
  if (operations.length === 0 || !useDiffSync) {
    console.log('[RbxSync] Using full sync');

    // Read file tree
    const treeResult = await client.readTree(projectDir);
    if (!treeResult) {
      vscode.window.showErrorMessage('Failed to read files.');
      return;
    }

    console.log('[RbxSync] Read tree:', treeResult.instances?.length || 0, 'instances');

    // Build set of file paths
    filePaths = new Set<string>();
    for (const inst of treeResult.instances) {
      if (inst.path) {
        filePaths.add(inst.path);
      }
    }

    // Create update operations for all file instances
    for (const inst of treeResult.instances) {
      operations.push({
        type: 'update',
        path: inst.path,
        data: inst
      });
    }

    // Delete orphaned instances using studio paths
    if (deleteOrphans) {
      const skipClasses = new Set([
        'Terrain', 'Camera', 'Workspace', 'ReplicatedStorage', 'ReplicatedFirst',
        'ServerScriptService', 'ServerStorage', 'StarterGui', 'StarterPack',
        'StarterPlayer', 'Lighting', 'SoundService', 'Teams', 'Chat',
        'LocalizationService', 'TestService', 'Players', 'RunService'
      ]);
      const skipNames = new Set(['SpawnLocation', 'Camera']);

      try {
        const studioPaths = await client.getStudioPaths();
        console.log('[RbxSync] Studio paths:', studioPaths?.length || 0);
        if (studioPaths && studioPaths.length > 0) {
          let deleteCount = 0;
          for (const studioPath of studioPaths) {
            // Skip if path is in file tree
            if (filePaths.has(studioPath)) continue;

            // Get the instance name (last part of path)
            const pathParts = studioPath.split('/');
            const instanceName = pathParts[pathParts.length - 1];

            // Skip services and special instances
            if (skipClasses.has(instanceName) || skipNames.has(instanceName)) continue;

            // Skip the service itself (top level)
            if (pathParts.length === 1) continue;

            // For direct service children (like Workspace/Baseplate), delete if service is in files
            // For deeper paths, only delete if parent exists in files
            const parentPath = studioPath.substring(0, studioPath.lastIndexOf('/'));
            const parentInFiles = parentPath && filePaths.has(parentPath);
            const isDirectServiceChild = pathParts.length === 2 && skipClasses.has(pathParts[0]);

            if (parentInFiles || isDirectServiceChild) {
              operations.push({
                type: 'delete',
                path: studioPath
              });
              deleteCount++;
              console.log('[RbxSync] Will delete orphan:', studioPath);
            }
          }
          console.log('[RbxSync] Delete operations:', deleteCount);
        }
      } catch (err) {
        console.error('[RbxSync] Failed to get studio paths:', err);
      }
    }
  }

  // Sync terrain if it exists
  try {
    const terrainResult = await client.readTerrain(projectDir);
    if (terrainResult?.terrain) {
      console.log('[RbxSync] Adding terrain sync operation');
      operations.push({
        type: 'update',
        path: 'Workspace/Terrain',
        data: {
          path: 'Workspace/Terrain',
          className: 'Terrain',
          name: 'Terrain',
          terrain: terrainResult.terrain
        }
      });
    }
  } catch (err) {
    console.error('[RbxSync] Failed to read terrain:', err);
  }

  if (operations.length === 0) {
    const showNotifications = config.get('showNotifications');
    if (showNotifications) {
      vscode.window.showInformationMessage('Already in sync');
    }
    return;
  }

  console.log('[RbxSync] Sending', operations.length, 'operations');
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const syncResult = await client.syncBatch(operations as any, projectDir);

  if (!syncResult) {
    vscode.window.showErrorMessage('Sync failed. Try again.');
    return;
  }

  // Mark the sync time for incremental sync
  if (useDiffSync) {
    await client.syncIncremental(projectDir, true);
  }

  console.log('[RbxSync] Sync result:', syncResult);

  if (syncResult.success) {
    if (config.get('showNotifications')) {
      const applied = syncResult.applied || 0;
      const skipped = syncResult.skipped || 0;

      if (applied === 0) {
        vscode.window.showInformationMessage(`Already in sync (${skipped} instances checked)`);
      } else if (skipped > 0) {
        vscode.window.showInformationMessage(`Applied ${applied} changes (${skipped} unchanged)`);
      } else {
        vscode.window.showInformationMessage(`Applied ${applied} changes`);
      }
    }
  } else if (syncResult.errors?.length) {
    console.error('[RbxSync] Sync errors:', syncResult.errors);
    vscode.window.showWarningMessage(`Synced with ${syncResult.errors.length} error(s)`);
  }
}
