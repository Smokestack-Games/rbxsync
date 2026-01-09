/**
 * rbxjson Language Server
 *
 * Main entry point for the LSP server providing completions, hover, and diagnostics.
 */

import {
  createConnection,
  TextDocuments,
  ProposedFeatures,
  InitializeParams,
  InitializeResult,
  TextDocumentSyncKind,
  CompletionItem,
  Hover,
  Diagnostic,
  DidChangeConfigurationNotification,
} from 'vscode-languageserver/node';
import { TextDocument } from 'vscode-languageserver-textdocument';

import { getAPIDumpHandler, APIDumpHandler } from './apiDump';
import { CompletionProvider } from './completionProvider';
import { HoverProvider } from './hoverProvider';
import { DiagnosticsProvider } from './diagnosticsProvider';

// Create connection and document manager
const connection = createConnection(ProposedFeatures.all);
const documents = new TextDocuments(TextDocument);

// Providers (initialized after API dump loads)
let completionProvider: CompletionProvider | null = null;
let hoverProvider: HoverProvider | null = null;
let diagnosticsProvider: DiagnosticsProvider | null = null;

// Configuration
let hasConfigurationCapability = false;
let hasWorkspaceFolderCapability = false;

/**
 * Initialize the language server
 */
connection.onInitialize(async (params: InitializeParams): Promise<InitializeResult> => {
  const capabilities = params.capabilities;

  hasConfigurationCapability = !!(
    capabilities.workspace && !!capabilities.workspace.configuration
  );
  hasWorkspaceFolderCapability = !!(
    capabilities.workspace && !!capabilities.workspace.workspaceFolders
  );

  // Initialize API dump
  const apiDump = getAPIDumpHandler();
  try {
    await apiDump.initialize();
    initializeProviders(apiDump);
  } catch (error) {
    connection.console.error(`Failed to initialize API dump: ${error}`);
  }

  const result: InitializeResult = {
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Incremental,
      completionProvider: {
        resolveProvider: false,
        triggerCharacters: ['"', ':', '{', ','],
      },
      hoverProvider: true,
      // Using push-model diagnostics via onDidChangeContent
    },
  };

  if (hasWorkspaceFolderCapability) {
    result.capabilities.workspace = {
      workspaceFolders: {
        supported: true,
      },
    };
  }

  return result;
});

/**
 * After initialization
 */
connection.onInitialized(() => {
  if (hasConfigurationCapability) {
    connection.client.register(DidChangeConfigurationNotification.type, undefined);
  }

  connection.console.log('[rbxjson LSP] Language server initialized');
});

/**
 * Initialize providers with API dump
 */
function initializeProviders(apiDump: APIDumpHandler): void {
  completionProvider = new CompletionProvider(apiDump);
  hoverProvider = new HoverProvider(apiDump);
  diagnosticsProvider = new DiagnosticsProvider(apiDump);
  connection.console.log('[rbxjson LSP] Providers initialized');
}

/**
 * Handle completion requests (with error handling to prevent VS Code sounds)
 */
connection.onCompletion((params) => {
  try {
    if (!completionProvider) {
      return [];
    }

    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return [];
    }

    return completionProvider.getCompletions(document, params.position);
  } catch (error) {
    connection.console.error(`Completion error: ${error}`);
    return [];
  }
});

/**
 * Handle hover requests (with error handling to prevent VS Code sounds)
 */
connection.onHover((params): Hover | null => {
  try {
    if (!hoverProvider) {
      return null;
    }

    const document = documents.get(params.textDocument.uri);
    if (!document) {
      return null;
    }

    return hoverProvider.getHover(document, params.position);
  } catch (error) {
    connection.console.error(`Hover error: ${error}`);
    return null;
  }
});

/**
 * Validate on save only (not while typing) - like Luau LSP
 */
documents.onDidSave((event) => {
  validateDocument(event.document);
});

/**
 * Validate a document and send diagnostics
 */
async function validateDocument(document: TextDocument): Promise<void> {
  if (!diagnosticsProvider) {
    return;
  }

  // Only validate .rbxjson files
  if (!document.uri.endsWith('.rbxjson')) {
    return;
  }

  const diagnostics: Diagnostic[] = diagnosticsProvider.validate(document);
  connection.sendDiagnostics({ uri: document.uri, diagnostics });
}

/**
 * Clear diagnostics when document is closed
 */
documents.onDidClose((e) => {
  connection.sendDiagnostics({ uri: e.document.uri, diagnostics: [] });
});

// Start listening
documents.listen(connection);
connection.listen();
