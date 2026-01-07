import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { RbxSyncClient } from './server/client';
import { StatusBarManager } from './views/statusBar';
import { ActivityViewProvider } from './views/activityView';
import { connectCommand, disconnectCommand } from './commands/connect';
import { extractCommand } from './commands/extract';
import { syncCommand } from './commands/sync';
import { runPlayTest, disposeTestChannel } from './commands/test';

let client: RbxSyncClient;
let statusBar: StatusBarManager;
let activityView: ActivityViewProvider;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  // Get configuration
  const config = vscode.workspace.getConfiguration('rbxsync');
  const port = config.get<number>('serverPort') || 44755;
  const autoConnect = config.get<boolean>('autoConnect') ?? true;

  // Initialize components
  client = new RbxSyncClient(port);

  // Set project directory for multi-workspace support
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (workspaceFolders?.length) {
    client.setProjectDir(workspaceFolders[0].uri.fsPath);
  }

  statusBar = new StatusBarManager(client);
  activityView = new ActivityViewProvider();

  // Register activity view
  const activityTreeView = vscode.window.createTreeView('rbxsync.activityView', {
    treeDataProvider: activityView
  });

  // Listen for connection changes and fetch all places
  client.onConnectionChange(async (state) => {
    if (state.connected) {
      // Fetch all connected places
      const places = await client.getConnectedPlaces();
      const projectDir = client.projectDir;

      // Update both status bar and activity view with all places
      statusBar.updatePlaces(places, projectDir);
      activityView.setConnectionStatus('connected', places, projectDir);
    } else {
      statusBar.updatePlaces([], '');
      activityView.setConnectionStatus('disconnected', [], '');
    }
  });

  // Register commands
  const commands = [
    vscode.commands.registerCommand('rbxsync.connect', async () => {
      activityView.setConnectionStatus('connecting');
      await connectCommand(client, statusBar);
    }),

    vscode.commands.registerCommand('rbxsync.disconnect', async () => {
      await disconnectCommand(client, statusBar);
    }),

    vscode.commands.registerCommand('rbxsync.extract', async () => {
      statusBar.setBusy('Extracting');
      activityView.setCurrentOperation('Extracting');
      await extractCommand(client, statusBar, activityView);
      activityView.setCurrentOperation(null);
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.sync', async () => {
      statusBar.setBusy('Syncing');
      activityView.setCurrentOperation('Syncing');
      await syncCommand(client, statusBar);
      activityView.setCurrentOperation(null);
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.runTest', async () => {
      statusBar.setBusy('Testing');
      activityView.setCurrentOperation('Testing');
      await runPlayTest(client);
      activityView.setCurrentOperation(null);
      statusBar.clearBusy();
    }),

    // Per-studio commands (take projectDir as argument)
    vscode.commands.registerCommand('rbxsync.syncTo', async (projectDir: string) => {
      statusBar.setBusy('Syncing');
      activityView.setCurrentOperation(`Syncing to ${projectDir}`);
      await syncCommand(client, statusBar, projectDir);
      activityView.setCurrentOperation(null);
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.extractFrom', async (projectDir: string) => {
      statusBar.setBusy('Extracting');
      activityView.setCurrentOperation(`Extracting from ${projectDir}`);
      await extractCommand(client, statusBar, activityView, projectDir);
      activityView.setCurrentOperation(null);
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.runTestOn', async (projectDir: string) => {
      statusBar.setBusy('Testing');
      activityView.setCurrentOperation(`Testing ${projectDir}`);
      await runPlayTest(client, projectDir);
      activityView.setCurrentOperation(null);
      statusBar.clearBusy();
    }),

    vscode.commands.registerCommand('rbxsync.refresh', () => {
      activityView.refresh();
    }),

    vscode.commands.registerCommand('rbxsync.openMetadata', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;

      const currentFile = editor.document.uri.fsPath;
      const ext = path.extname(currentFile);

      if (ext !== '.lua' && ext !== '.luau') return;

      const baseName = path.basename(currentFile)
        .replace('.server.luau', '')
        .replace('.client.luau', '')
        .replace('.luau', '')
        .replace('.server.lua', '')
        .replace('.client.lua', '')
        .replace('.lua', '');

      const dir = path.dirname(currentFile);
      const metadataFile = path.join(dir, `${baseName}.rbxjson`);

      if (fs.existsSync(metadataFile)) {
        const doc = await vscode.workspace.openTextDocument(metadataFile);
        await vscode.window.showTextDocument(doc, { viewColumn: vscode.ViewColumn.Beside });
      }
    }),

    vscode.commands.registerCommand('rbxsync.toggleMetadataFiles', async () => {
      const filesConfig = vscode.workspace.getConfiguration('files');
      const exclude = filesConfig.get<Record<string, boolean>>('exclude') || {};
      const isHidden = exclude['**/*.rbxjson'] === true;

      exclude['**/*.rbxjson'] = !isHidden;
      await filesConfig.update('exclude', exclude, vscode.ConfigurationTarget.Workspace);
    })
  ];

  // Add to subscriptions
  context.subscriptions.push(
    client,
    statusBar,
    activityView,
    activityTreeView,
    ...commands
  );

  // Show status bar
  statusBar.show();

  // Auto-connect if enabled
  if (autoConnect) {
    setTimeout(async () => {
      activityView.setConnectionStatus('connecting');
      const connected = await client.connect();
      if (connected) {
        // Register workspace with server immediately
        if (client.projectDir) {
          await client.registerWorkspace(client.projectDir);
          statusBar.updatePlaces([], client.projectDir); // Set currentProjectDir for polling
        }
        statusBar.startPolling();
      }
    }, 1000);
  }
}

export function deactivate(): void {
  disposeTestChannel();
}
