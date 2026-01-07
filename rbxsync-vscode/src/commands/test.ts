import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { ConsoleMessage } from '../server/types';

// Output channel for test results
let outputChannel: vscode.OutputChannel | null = null;

function getOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    outputChannel = vscode.window.createOutputChannel('RbxSync Tests');
  }
  return outputChannel;
}

function getMessagePrefix(type: string): string {
  switch (type) {
    case 'MessageError':
      return '[ERROR]';
    case 'MessageWarning':
      return '[WARN]';
    case 'MessageInfo':
      return '[INFO]';
    default:
      return '[OUT]';
  }
}

function formatConsoleOutput(messages: ConsoleMessage[]): string[] {
  const lines: string[] = [];
  for (const msg of messages) {
    const prefix = getMessagePrefix(msg.type);
    lines.push(`${prefix} [${msg.timestamp.toFixed(3)}s] ${msg.message}`);
  }
  return lines;
}

function getSummary(messages: ConsoleMessage[]): { errors: number; warnings: number; total: number } {
  const errors = messages.filter(m => m.type === 'MessageError').length;
  const warnings = messages.filter(m => m.type === 'MessageWarning').length;
  return { errors, warnings, total: messages.length };
}

/**
 * Run an automated play test in Roblox Studio
 * This starts a play session, waits for it to complete, and returns all console output
 */
export async function runPlayTest(client: RbxSyncClient): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('RbxSync: Not connected to server. Connect first.');
    return;
  }

  const channel = getOutputChannel();
  channel.show(true);
  channel.clear();

  // Ask for duration
  const durationStr = await vscode.window.showInputBox({
    prompt: 'Test duration in seconds',
    value: '5',
    validateInput: (value) => {
      const num = parseInt(value, 10);
      if (isNaN(num) || num < 1 || num > 300) {
        return 'Enter a number between 1 and 300';
      }
      return null;
    }
  });

  if (!durationStr) {
    return; // User cancelled
  }

  const duration = parseInt(durationStr, 10);

  channel.appendLine('');
  channel.appendLine('='.repeat(60));
  channel.appendLine(`Starting automated play test (${duration}s)...`);
  channel.appendLine('Studio will enter play mode automatically.');
  channel.appendLine('='.repeat(60));
  channel.appendLine('');

  // Start the test
  const startResult = await client.runTest(duration, 'Play');
  if (!startResult || !startResult.success) {
    const errorMsg = startResult?.message || 'Unknown error';
    vscode.window.showErrorMessage(`RbxSync: Failed to start test - ${errorMsg}`);
    channel.appendLine(`[ERROR] Failed to start test: ${errorMsg}`);
    return;
  }

  channel.appendLine('[INFO] Test started - capturing console output...');
  channel.appendLine('');

  // Poll for completion with progress
  const pollInterval = 500; // ms
  const maxWait = (duration + 10) * 1000; // Give extra time
  const startTime = Date.now();
  let lastMessageCount = 0;

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: 'Running play test...',
      cancellable: true
    },
    async (progress, token) => {
      while (Date.now() - startTime < maxWait) {
        if (token.isCancellationRequested) {
          channel.appendLine('[INFO] Test cancelled by user');
          break;
        }

        await new Promise(resolve => setTimeout(resolve, pollInterval));

        const status = await client.getTestStatus();
        if (!status) {
          continue;
        }

        // Show new messages as they come in
        if (status.totalMessages > lastMessageCount) {
          const newMessages = status.output.slice(lastMessageCount);
          for (const line of formatConsoleOutput(newMessages)) {
            channel.appendLine(line);
          }
          lastMessageCount = status.totalMessages;
        }

        // Update progress
        const elapsed = (Date.now() - startTime) / 1000;
        progress.report({
          message: `${elapsed.toFixed(1)}s - ${status.totalMessages} messages`
        });

        // Check if complete
        if (status.complete || !status.inProgress) {
          break;
        }
      }
    }
  );

  // Get final results
  const result = await client.finishTest();
  if (!result) {
    vscode.window.showErrorMessage('RbxSync: Failed to get test results');
    return;
  }

  // Show any remaining messages
  if (result.output.length > lastMessageCount) {
    const newMessages = result.output.slice(lastMessageCount);
    for (const line of formatConsoleOutput(newMessages)) {
      channel.appendLine(line);
    }
  }

  // Summary
  channel.appendLine('');
  channel.appendLine('='.repeat(60));
  channel.appendLine(`Test completed in ${result.duration?.toFixed(2) || '?'}s`);
  const summary = getSummary(result.output);
  channel.appendLine(`Total: ${summary.total} | Errors: ${summary.errors} | Warnings: ${summary.warnings}`);
  channel.appendLine('='.repeat(60));

  if (result.error) {
    channel.appendLine(`[ERROR] Test error: ${result.error}`);
    vscode.window.showErrorMessage(`RbxSync: Test failed - ${result.error}`);
  } else if (summary.errors > 0) {
    vscode.window.showWarningMessage(`RbxSync: Test complete - ${summary.errors} error(s) found`);
  } else {
    vscode.window.showInformationMessage(`RbxSync: Test complete - ${summary.total} messages captured`);
  }
}

/**
 * Start manual console capture (old behavior for backward compatibility)
 * User must manually press Play in Studio
 */
export async function startTestCapture(client: RbxSyncClient): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('RbxSync: Not connected to server. Connect first.');
    return;
  }

  const channel = getOutputChannel();
  channel.show(true);
  channel.appendLine('');
  channel.appendLine('='.repeat(60));
  channel.appendLine('Starting manual console capture...');
  channel.appendLine('Press Play in Roblox Studio to run your game.');
  channel.appendLine('Use "Stop Capture" command when done.');
  channel.appendLine('='.repeat(60));
  channel.appendLine('');

  // Use the old direct endpoint for manual capture
  try {
    const response = await fetch('http://127.0.0.1:44755/test/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}'
    });

    const result = await response.json() as { success: boolean; message?: string; error?: string };

    if (result.success) {
      vscode.window.showInformationMessage('RbxSync: Console capture started - click Play in Studio');
    } else {
      vscode.window.showErrorMessage(`RbxSync: ${result.error || 'Failed to start capture'}`);
    }
  } catch (error) {
    vscode.window.showErrorMessage(`RbxSync: Failed to start capture - ${error}`);
  }
}

/**
 * Stop manual capture and show all output
 */
export async function stopTestCapture(client: RbxSyncClient): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('RbxSync: Not connected to server.');
    return;
  }

  const channel = getOutputChannel();

  try {
    const response = await fetch('http://127.0.0.1:44755/test/stop', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}'
    });

    const result = await response.json() as {
      success: boolean;
      output: ConsoleMessage[];
      totalMessages: number;
      duration?: number;
      error?: string;
    };

    channel.appendLine('');
    channel.appendLine('='.repeat(60));
    channel.appendLine(`Capture stopped - ${result.totalMessages} messages captured`);
    if (result.duration) {
      channel.appendLine(`Duration: ${result.duration.toFixed(2)}s`);
    }
    channel.appendLine('='.repeat(60));
    channel.appendLine('');

    // Show output
    if (result.output && result.output.length > 0) {
      for (const line of formatConsoleOutput(result.output)) {
        channel.appendLine(line);
      }
    } else {
      channel.appendLine('(No console output captured)');
    }

    // Summary
    const summary = getSummary(result.output || []);
    channel.appendLine('');
    channel.appendLine('--- Summary ---');
    channel.appendLine(`Total: ${summary.total} | Errors: ${summary.errors} | Warnings: ${summary.warnings}`);

    if (summary.errors > 0) {
      vscode.window.showWarningMessage(`RbxSync: Capture complete - ${summary.errors} errors found`);
    } else {
      vscode.window.showInformationMessage(`RbxSync: Capture complete - ${summary.total} messages`);
    }
  } catch (error) {
    vscode.window.showErrorMessage(`RbxSync: Failed to stop capture - ${error}`);
  }
}

/**
 * Poll current output without stopping (for manual capture mode)
 */
export async function getTestOutput(client: RbxSyncClient): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('RbxSync: Not connected to server.');
    return;
  }

  const channel = getOutputChannel();

  try {
    const response = await fetch('http://127.0.0.1:44755/test/status');
    const result = await response.json() as {
      capturing: boolean;
      output: ConsoleMessage[];
      totalMessages: number;
    };

    if (!result.capturing) {
      channel.appendLine('[Not capturing - start capture first]');
      return;
    }

    channel.appendLine('');
    channel.appendLine(`--- Current output (${result.totalMessages} messages) ---`);

    if (result.output && result.output.length > 0) {
      for (const line of formatConsoleOutput(result.output)) {
        channel.appendLine(line);
      }
    }
  } catch (error) {
    vscode.window.showErrorMessage(`RbxSync: Failed to get output - ${error}`);
  }
}

export function disposeTestChannel(): void {
  if (outputChannel) {
    outputChannel.dispose();
    outputChannel = null;
  }
}
