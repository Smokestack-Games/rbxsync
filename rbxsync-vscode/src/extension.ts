import * as vscode from 'vscode';
import { RbxSyncClient } from './server/client';
import { StatusBarManager } from './views/statusBar';
import { SidebarWebviewProvider } from './views/sidebarWebview';
import { connectCommand, disconnectCommand, initServerTerminal, disposeServerTerminal } from './commands/connect';
import { extractCommand } from './commands/extract';
import { syncCommand } from './commands/sync';
import { runPlayTest, disposeTestChannel } from './commands/test';
import { openConsole, closeConsole, toggleE2EMode, initE2EMode, initConsole, disposeConsole, isE2EMode } from './commands/console';
import { initTrashSystem, recoverDeletedFolder } from './commands/trash';

let client: RbxSyncClient;
let statusBar: StatusBarManager;
let sidebarView: SidebarWebviewProvider;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  // Get configuration
  const config = vscode.workspace.getConfiguration('rbxsync');
  const port = config.get<number>('serverPort') || 44755;
  const autoConnect = config.get<boolean>('autoConnect') ?? true;

  // Initialize context variable for command enablement (RBXSYNC-72)
  vscode.commands.executeCommand('setContext', 'rbxsync.connected', false);

  // Initialize components
  client = new RbxSyncClient(port);

  // Initialize E2E mode from saved state
  initE2EMode(context);

  // Initialize console terminal tracking (handles terminal close events)
  initConsole(context);

  // Initialize server terminal tracking (reuses terminal instead of spawning new ones - RBXSYNC-71)
  const serverTerminalDisposable = initServerTerminal();
  context.subscriptions.push(serverTerminalDisposable);

  // Prompt to enable RbxSync icon theme on first install
  const iconThemePromptShown = context.globalState.get('iconThemePromptShown');
  if (!iconThemePromptShown) {
    const result = await vscode.window.showInformationMessage(
      'RbxSync: Enable custom Roblox file icons?',
      'Yes',
      'No'
    );
    if (result === 'Yes') {
      await vscode.workspace.getConfiguration().update(
        'workbench.iconTheme',
        'rbxsync-icons',
        vscode.ConfigurationTarget.Global
      );
    }
    await context.globalState.update('iconThemePromptShown', true);
  }

  // Set project directory for multi-workspace support
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (workspaceFolders?.length) {
    client.setProjectDir(workspaceFolders[0].uri.fsPath);
  }

  statusBar = new StatusBarManager(client);

  // Get extension version from package.json
  const extensionVersion = context.extension.packageJSON.version || '1.1.0';
  sidebarView = new SidebarWebviewProvider(context.extensionUri, extensionVersion);

  // Initialize rbxjson hidden state from config
  const filesConfig = vscode.workspace.getConfiguration('files');
  const exclude = filesConfig.get<Record<string, boolean>>('exclude') || {};
  sidebarView.setRbxjsonHidden(exclude['**/*.rbxjson'] === true);

  // Register webview sidebar view
  const sidebarViewDisposable = vscode.window.registerWebviewViewProvider(
    SidebarWebviewProvider.viewType,
    sidebarView
  );

  // Listen for connection changes and fetch all places
  client.onConnectionChange(async (state) => {
    if (state.connected) {
      // Set context variable for command enablement
      vscode.commands.executeCommand('setContext', 'rbxsync.connected', true);

      // Fetch all connected places
      const places = await client.getConnectedPlaces();
      const projectDir = client.projectDir;

      // Update both status bar and activity view with all places
      statusBar.updatePlaces(places, projectDir);
      sidebarView.setConnectionStatus('connected', places, projectDir);
    } else {
      // Clear context variable when disconnected
      vscode.commands.executeCommand('setContext', 'rbxsync.connected', false);

      statusBar.updatePlaces([], '');
      sidebarView.setConnectionStatus('disconnected', [], '');
    }
  });

  // Listen for places changes from polling and update sidebar
  statusBar.onPlacesChange((places, projectDir) => {
    sidebarView.updatePlaces(places, projectDir);
  });

  // Listen for server-initiated operation status changes (RBXSYNC-77)
  // This makes the UI react to CLI/MCP commands
  statusBar.onOperationStatusChange((operation) => {
    sidebarView.handleServerOperation(operation);
  });

  // Register commands
  const commands = [
    vscode.commands.registerCommand('rbxsync.connect', async () => {
      sidebarView.setConnectionStatus('connecting');
      await connectCommand(client, statusBar);
    }),

    vscode.commands.registerCommand('rbxsync.disconnect', async () => {
      await disconnectCommand(client, statusBar);
    }),

    vscode.commands.registerCommand('rbxsync.extract', async () => {
      if (isE2EMode()) { await openConsole(client); }
      statusBar.setBusy('Extracting');
      // Get placeId and sessionId from connected place if available
      const place = client.connectionState.place;
      const placeId = place?.place_id;
      const sessionId = place?.session_id;
      if (placeId !== undefined) {
        sidebarView.startStudioOperation(placeId, 'extract', sessionId);
      }
      try {
        await extractCommand(client, statusBar, sidebarView, undefined, placeId, sessionId);
      } finally {
        statusBar.clearBusy();
      }
    }),

    vscode.commands.registerCommand('rbxsync.sync', async () => {
      if (isE2EMode()) { await openConsole(client); }
      statusBar.setBusy('Syncing');
      // Get placeId and sessionId from connected place if available
      const place = client.connectionState.place;
      const placeId = place?.place_id;
      const sessionId = place?.session_id;
      if (placeId !== undefined) {
        sidebarView.startStudioOperation(placeId, 'sync', sessionId);
      }
      try {
        await syncCommand(client, statusBar);
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, true, 'Sync complete', sessionId);
        }
      } catch (e) {
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, false, 'Sync failed', sessionId);
        }
      } finally {
        statusBar.clearBusy();
      }
    }),

    vscode.commands.registerCommand('rbxsync.runTest', async () => {
      if (isE2EMode()) { await openConsole(client); }
      statusBar.setBusy('Testing');
      // Get placeId and sessionId from connected place if available
      const place = client.connectionState.place;
      const placeId = place?.place_id;
      const sessionId = place?.session_id;
      if (placeId !== undefined) {
        sidebarView.startStudioOperation(placeId, 'test', sessionId);
      }
      try {
        await runPlayTest(client);
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, true, 'Test complete', sessionId);
        }
      } catch (e) {
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, false, 'Test failed', sessionId);
        }
      } finally {
        statusBar.clearBusy();
      }
    }),

    // Per-studio commands (take projectDir, placeId, and sessionId as arguments)
    vscode.commands.registerCommand('rbxsync.syncTo', async (projectDir: string, placeId?: number, sessionId?: string | null) => {
      if (isE2EMode()) { await openConsole(client); }
      statusBar.setBusy('Syncing');
      if (placeId !== undefined) {
        sidebarView.startStudioOperation(placeId, 'sync', sessionId);
      }
      try {
        await syncCommand(client, statusBar, projectDir);
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, true, 'Sync complete', sessionId);
        }
      } catch (e) {
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, false, 'Sync failed', sessionId);
        }
      }
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.extractFrom', async (projectDir: string, placeId?: number, sessionId?: string | null) => {
      if (isE2EMode()) { await openConsole(client); }
      statusBar.setBusy('Extracting');
      if (placeId !== undefined) {
        sidebarView.startStudioOperation(placeId, 'extract', sessionId);
      }
      try {
        await extractCommand(client, statusBar, sidebarView, projectDir, placeId, sessionId);
        // extractCommand will call logExtract which completes the operation
      } catch (e) {
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, false, 'Extract failed', sessionId);
        }
      }
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.runTestOn', async (projectDir: string, placeId?: number, sessionId?: string | null) => {
      if (isE2EMode()) { await openConsole(client); }
      statusBar.setBusy('Testing');
      if (placeId !== undefined) {
        sidebarView.startStudioOperation(placeId, 'test', sessionId);
      }
      try {
        await runPlayTest(client, projectDir);
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, true, 'Test complete', sessionId);
        }
      } catch (e) {
        if (placeId !== undefined) {
          sidebarView.completeStudioOperation(placeId, false, 'Test failed', sessionId);
        }
      }
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.linkStudio', async (placeId: number) => {
      const success = await client.linkStudio(placeId);
      if (success) {
        // Refresh the sidebar to show updated link status
        const places = await client.getConnectedPlaces();
        sidebarView.updatePlaces(places, client.projectDir);
      }
    }),

    vscode.commands.registerCommand('rbxsync.unlinkStudio', async (placeId: number) => {
      const success = await client.unlinkStudio(placeId);
      if (success) {
        // Refresh the sidebar to show updated link status
        const places = await client.getConnectedPlaces();
        sidebarView.updatePlaces(places, client.projectDir);
      }
    }),

    vscode.commands.registerCommand('rbxsync.refresh', () => {
      sidebarView.refresh();
    }),

    vscode.commands.registerCommand('rbxsync.toggleMetadataFiles', async () => {
      const filesConfig = vscode.workspace.getConfiguration('files');
      const exclude = filesConfig.get<Record<string, boolean>>('exclude') || {};
      const isHidden = exclude['**/*.rbxjson'] === true;

      exclude['**/*.rbxjson'] = !isHidden;
      await filesConfig.update('exclude', exclude, vscode.ConfigurationTarget.Workspace);
      sidebarView.setRbxjsonHidden(!isHidden);
    }),

    // Console commands for E2E testing
    vscode.commands.registerCommand('rbxsync.openConsole', async () => {
      await openConsole(client);
    }),

    vscode.commands.registerCommand('rbxsync.closeConsole', () => {
      closeConsole();
    }),

    vscode.commands.registerCommand('rbxsync.toggleE2EMode', () => {
      const enabled = toggleE2EMode(context);
      sidebarView.setE2EMode(enabled);
    }),

    // Undo extraction command
    vscode.commands.registerCommand('rbxsync.undoExtract', async () => {
      await client.undoExtract();
    }),

    // Trash recovery command
    vscode.commands.registerCommand('rbxsync.recoverDeleted', async () => {
      await recoverDeletedFolder();
    }),

    // Settings command
    vscode.commands.registerCommand('rbxsync.openSettings', () => {
      vscode.commands.executeCommand('workbench.action.openSettings', '@ext:rbxsync.rbxsync');
    }),

    // Toggle cat visibility
    vscode.commands.registerCommand('rbxsync.toggleCat', () => {
      sidebarView.toggleCat();
    })
  ];

  // Initialize trash system for folder recovery
  initTrashSystem(context);

  // Add to subscriptions
  context.subscriptions.push(
    client,
    statusBar,
    sidebarViewDisposable,
    ...commands
  );

  // Show status bar
  statusBar.show();

  // Auto-connect if enabled
  if (autoConnect) {
    setTimeout(async () => {
      sidebarView.setConnectionStatus('connecting');
      const connected = await client.connect();
      if (connected) {
        // Register workspace with server immediately
        if (client.projectDir) {
          await client.registerWorkspace(client.projectDir);
          statusBar.updatePlaces([], client.projectDir); // Set currentProjectDir for polling
        }
      } else {
        // Set project dir for polling even if not connected yet
        if (client.projectDir) {
          statusBar.updatePlaces([], client.projectDir);
        }
      }
      // Always start polling to detect server when it starts
      statusBar.startPolling();
    }, 1000);
  }
}

export async function deactivate(): Promise<void> {
  disposeTestChannel();
  disposeConsole();
  disposeServerTerminal();
}
