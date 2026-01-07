import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import { ConsoleMessage } from '../server/types';

let outputChannel: vscode.OutputChannel | null = null;

function getOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    outputChannel = vscode.window.createOutputChannel('RbxSync');
  }
  return outputChannel;
}

function formatMessage(msg: ConsoleMessage): string {
  const ts = `[${msg.timestamp.toFixed(2)}s]`;
  switch (msg.type) {
    case 'MessageError':
      return `${ts} \u2717 ${msg.message}`;
    case 'MessageWarning':
      return `${ts} \u26A0 ${msg.message}`;
    default:
      return `${ts} ${msg.message}`;
  }
}

function getSummary(messages: ConsoleMessage[]): { errors: number; warnings: number; total: number } {
  return {
    errors: messages.filter(m => m.type === 'MessageError').length,
    warnings: messages.filter(m => m.type === 'MessageWarning').length,
    total: messages.length
  };
}

export async function runPlayTest(client: RbxSyncClient, targetProjectDir?: string): Promise<void> {
  if (!client.connectionState.connected) {
    vscode.window.showErrorMessage('Not connected. Is Studio running?');
    return;
  }

  // Store projectDir for routing to specific Studio
  const projectDir = targetProjectDir;

  // Quick duration input
  const durationStr = await vscode.window.showInputBox({
    prompt: 'Duration (seconds)',
    value: '5',
    validateInput: v => {
      const n = parseInt(v, 10);
      return (isNaN(n) || n < 1 || n > 300) ? '1-300' : null;
    }
  });

  if (!durationStr) return;

  const duration = parseInt(durationStr, 10);
  const channel = getOutputChannel();
  channel.show(true);
  channel.clear();
  channel.appendLine(`Playing for ${duration}s...\n`);

  // Start test
  const startResult = await client.runTest(duration, 'Play', projectDir);
  if (!startResult?.success) {
    channel.appendLine(`\u2717 Failed to start: ${startResult?.message || 'Unknown error'}`);
    return;
  }

  // Poll for messages
  const startTime = Date.now();
  const maxWait = (duration + 10) * 1000;
  let lastCount = 0;

  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: 'Testing...', cancellable: true },
    async (progress, token) => {
      while (Date.now() - startTime < maxWait && !token.isCancellationRequested) {
        await new Promise(r => setTimeout(r, 500));

        const status = await client.getTestStatus(projectDir);
        if (!status) continue;

        // Stream new messages
        if (status.totalMessages > lastCount) {
          for (const msg of status.output.slice(lastCount)) {
            channel.appendLine(formatMessage(msg));
          }
          lastCount = status.totalMessages;
        }

        const elapsed = (Date.now() - startTime) / 1000;
        progress.report({ message: `${elapsed.toFixed(0)}s` });

        if (status.complete || !status.inProgress) break;
      }
    }
  );

  // Final results
  const result = await client.finishTest(projectDir);
  if (!result) return;

  // Show remaining messages
  for (const msg of result.output.slice(lastCount)) {
    channel.appendLine(formatMessage(msg));
  }

  // Summary line
  const summary = getSummary(result.output);
  channel.appendLine('');
  channel.appendLine(`Done: ${summary.total} messages`);
  if (summary.errors > 0 || summary.warnings > 0) {
    channel.appendLine(`     ${summary.errors} error(s), ${summary.warnings} warning(s)`);
  }

  // Notification only for errors
  if (result.error) {
    vscode.window.showErrorMessage(`Test failed: ${result.error}`);
  } else if (summary.errors > 0) {
    vscode.window.showWarningMessage(`Test done with ${summary.errors} error(s)`);
  }
}

export function disposeTestChannel(): void {
  outputChannel?.dispose();
  outputChannel = null;
}
