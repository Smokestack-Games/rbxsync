import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { RbxSyncClient } from './server/client';
import { StatusBarManager } from './views/statusBar';
import { InstanceTreeProvider } from './views/treeView';
import { GitTreeProvider } from './views/gitView';
import { connectCommand, disconnectCommand } from './commands/connect';
import { extractCommand } from './commands/extract';
import { syncCommand, gitStatusCommand, gitCommitCommand } from './commands/sync';
import { runPlayTest, startTestCapture, stopTestCapture, getTestOutput, disposeTestChannel } from './commands/test';

let client: RbxSyncClient;
let statusBar: StatusBarManager;
let instanceTree: InstanceTreeProvider;
let gitTree: GitTreeProvider;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  console.log('RbxSync extension activating...');

  // Get configuration
  const config = vscode.workspace.getConfiguration('rbxsync');
  const port = config.get<number>('serverPort') || 44755;
  const autoConnect = config.get<boolean>('autoConnect') ?? true;

  // Initialize components
  client = new RbxSyncClient(port);
  statusBar = new StatusBarManager(client);
  instanceTree = new InstanceTreeProvider();
  gitTree = new GitTreeProvider();

  // Register tree views
  const instanceTreeView = vscode.window.createTreeView('rbxsync.instanceTree', {
    treeDataProvider: instanceTree,
    showCollapseAll: true
  });

  const gitTreeView = vscode.window.createTreeView('rbxsync.gitView', {
    treeDataProvider: gitTree
  });

  // Set project directory if workspace exists
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (workspaceFolders && workspaceFolders.length > 0) {
    const projectDir = workspaceFolders[0].uri.fsPath;
    instanceTree.setProjectDir(projectDir);
    gitTree.setProjectDir(projectDir);

    // Load initial git status
    updateGitStatus(projectDir);
  }

  // Register commands
  const commands = [
    vscode.commands.registerCommand('rbxsync.connect', () =>
      connectCommand(client, statusBar)
    ),
    vscode.commands.registerCommand('rbxsync.disconnect', () =>
      disconnectCommand(client, statusBar)
    ),
    vscode.commands.registerCommand('rbxsync.extract', () =>
      extractCommand(client, statusBar, instanceTree)
    ),
    vscode.commands.registerCommand('rbxsync.sync', () =>
      syncCommand(client, statusBar)
    ),
    vscode.commands.registerCommand('rbxsync.refresh', () => {
      instanceTree.refresh();
      if (workspaceFolders && workspaceFolders.length > 0) {
        updateGitStatus(workspaceFolders[0].uri.fsPath);
      }
    }),
    vscode.commands.registerCommand('rbxsync.openFile', (node: { filePath?: string }) => {
      if (node.filePath) {
        vscode.window.showTextDocument(vscode.Uri.file(node.filePath));
      }
    }),
    vscode.commands.registerCommand('rbxsync.gitStatus', () =>
      gitStatusCommand(client)
    ),
    vscode.commands.registerCommand('rbxsync.gitCommit', () =>
      gitCommitCommand(client)
    ),
    // Test runner commands for AI-powered development workflows
    vscode.commands.registerCommand('rbxsync.runTest', () =>
      runPlayTest(client)
    ),
    vscode.commands.registerCommand('rbxsync.testStart', () =>
      startTestCapture(client)
    ),
    vscode.commands.registerCommand('rbxsync.testStop', () =>
      stopTestCapture(client)
    ),
    vscode.commands.registerCommand('rbxsync.testOutput', () =>
      getTestOutput(client)
    )
  ];

  // Watch for file changes
  const fileWatcher = vscode.workspace.createFileSystemWatcher('**/*.{rbxjson,luau,lua}');
  fileWatcher.onDidChange(() => {
    instanceTree.refresh();
    if (workspaceFolders && workspaceFolders.length > 0) {
      updateGitStatus(workspaceFolders[0].uri.fsPath);
    }
  });
  fileWatcher.onDidCreate(() => instanceTree.refresh());
  fileWatcher.onDidDelete(() => instanceTree.refresh());

  // Add to subscriptions
  context.subscriptions.push(
    client,
    statusBar,
    instanceTree,
    gitTree,
    instanceTreeView,
    gitTreeView,
    fileWatcher,
    ...commands
  );

  // Show status bar
  statusBar.show();

  // Auto-connect if enabled
  if (autoConnect) {
    setTimeout(async () => {
      const connected = await client.connect();
      if (connected) {
        statusBar.startPolling();
      }
    }, 1000);
  }

  console.log('RbxSync extension activated');
}

async function updateGitStatus(projectDir: string): Promise<void> {
  if (!client.connectionState.connected) return;

  const status = await client.getGitStatus(projectDir);
  if (status) {
    gitTree.updateStatus(status);
  }
}

export function deactivate(): void {
  disposeTestChannel();
  console.log('RbxSync extension deactivated');
}
