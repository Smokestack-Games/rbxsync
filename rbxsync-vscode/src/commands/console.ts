import * as vscode from 'vscode';
import { RbxSyncClient } from '../server/client';
import * as http from 'http';

let consoleTerminal: vscode.Terminal | undefined;
let writeEmitter: vscode.EventEmitter<string> | undefined;
let sseRequest: http.ClientRequest | undefined;
let isE2EModeEnabled = false;

interface ConsoleMessage {
  timestamp: string;
  message_type: string;
  message: string;
  source?: string;
}

// ANSI color codes
const COLORS = {
  reset: '\x1b[0m',
  dim: '\x1b[2m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  white: '\x1b[37m',
};

function formatMessage(msg: ConsoleMessage): string {
  const typeColor = msg.message_type === 'error' ? COLORS.red
    : msg.message_type === 'warn' ? COLORS.yellow
    : COLORS.white;

  const typeLabel = msg.message_type === 'error' ? 'ERR'
    : msg.message_type === 'warn' ? 'WRN'
    : 'INF';

  return `${COLORS.dim}[${msg.timestamp}]${COLORS.reset} ${typeColor}[${typeLabel}]${COLORS.reset} ${msg.message}\r\n`;
}

/**
 * Create a pseudo-terminal that displays Studio console output
 */
function createConsoleTerminal(client: RbxSyncClient): vscode.Terminal {
  const writeEmitterLocal = new vscode.EventEmitter<string>();
  writeEmitter = writeEmitterLocal;

  const pty: vscode.Pseudoterminal = {
    onDidWrite: writeEmitterLocal.event,
    open: () => {
      writeEmitterLocal.fire(`${COLORS.cyan}━━━ RbxSync Console ━━━${COLORS.reset}\r\n`);
      writeEmitterLocal.fire(`${COLORS.dim}Streaming Studio console output...${COLORS.reset}\r\n\r\n`);
      startSSEStream(client);
    },
    close: () => {
      stopSSEStream();
    },
    handleInput: (data: string) => {
      // Ignore input - this is a read-only console
      if (data === '\x03') { // Ctrl+C
        writeEmitterLocal.fire(`${COLORS.dim}(Use Cmd+W to close)${COLORS.reset}\r\n`);
      }
    }
  };

  return vscode.window.createTerminal({
    name: 'RbxSync Console',
    pty,
    iconPath: new vscode.ThemeIcon('output')
  });
}

/**
 * Start streaming console messages via SSE
 */
function startSSEStream(client: RbxSyncClient): void {
  const url = `http://127.0.0.1:${client.port}/console/subscribe`;

  const req = http.request(url, {
    method: 'GET',
    headers: {
      'Accept': 'text/event-stream',
      'Cache-Control': 'no-cache',
    }
  }, (res) => {
    let buffer = '';

    res.on('data', (chunk: Buffer) => {
      buffer += chunk.toString();
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const jsonStr = line.slice(6);
          if (jsonStr === 'keepalive') continue;

          try {
            const msg: ConsoleMessage = JSON.parse(jsonStr);
            if (writeEmitter) {
              writeEmitter.fire(formatMessage(msg));
            }
          } catch (e) {
            // Ignore parse errors
          }
        }
      }
    });

    res.on('end', () => {
      if (writeEmitter) {
        writeEmitter.fire(`${COLORS.dim}[Stream ended - server disconnected]${COLORS.reset}\r\n`);
      }
    });
  });

  req.on('error', (err) => {
    if (writeEmitter) {
      writeEmitter.fire(`${COLORS.red}[Connection error: ${err.message}]${COLORS.reset}\r\n`);
    }
  });

  req.end();
  sseRequest = req;
}

/**
 * Stop SSE stream
 */
function stopSSEStream(): void {
  if (sseRequest) {
    sseRequest.destroy();
    sseRequest = undefined;
  }
}

/**
 * Open the console terminal
 */
export async function openConsole(client: RbxSyncClient): Promise<void> {
  // Reuse existing terminal if available and not disposed
  if (consoleTerminal && !consoleTerminal.exitStatus) {
    consoleTerminal.show();
    return;
  }

  // Only create new terminal if none exists or previous was closed
  if (consoleTerminal) {
    consoleTerminal.dispose();
  }

  consoleTerminal = createConsoleTerminal(client);
  consoleTerminal.show();
}

/**
 * Close the console terminal
 */
export function closeConsole(): void {
  if (consoleTerminal) {
    consoleTerminal.dispose();
    consoleTerminal = undefined;
  }
}

/**
 * Toggle E2E testing mode
 */
export function toggleE2EMode(context: vscode.ExtensionContext): boolean {
  isE2EModeEnabled = !isE2EModeEnabled;
  context.globalState.update('rbxsync.e2eMode', isE2EModeEnabled);

  if (isE2EModeEnabled) {
    vscode.window.showInformationMessage('RbxSync E2E Testing Mode enabled - console will auto-open on operations');
  } else {
    vscode.window.showInformationMessage('RbxSync E2E Testing Mode disabled');
  }

  return isE2EModeEnabled;
}

/**
 * Check if E2E mode is enabled
 */
export function isE2EMode(): boolean {
  return isE2EModeEnabled;
}

/**
 * Initialize E2E mode from saved state
 */
export function initE2EMode(context: vscode.ExtensionContext): void {
  isE2EModeEnabled = context.globalState.get('rbxsync.e2eMode', false);
}

/**
 * Initialize console terminal tracking
 * Registers listener to clean up reference when terminal is closed by user
 */
export function initConsole(context: vscode.ExtensionContext): void {
  const terminalCloseListener = vscode.window.onDidCloseTerminal((closedTerminal) => {
    if (closedTerminal === consoleTerminal) {
      stopSSEStream();
      consoleTerminal = undefined;
      if (writeEmitter) {
        writeEmitter.dispose();
        writeEmitter = undefined;
      }
    }
  });
  context.subscriptions.push(terminalCloseListener);
}

/**
 * Dispose resources
 */
export function disposeConsole(): void {
  stopSSEStream();
  if (consoleTerminal) {
    consoleTerminal.dispose();
    consoleTerminal = undefined;
  }
  if (writeEmitter) {
    writeEmitter.dispose();
    writeEmitter = undefined;
  }
}
