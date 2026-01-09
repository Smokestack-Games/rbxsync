const esbuild = require('esbuild');

const production = process.argv.includes('--production');
const watch = process.argv.includes('--watch');

const commonOptions = {
  bundle: true,
  external: ['vscode'],
  format: 'cjs',
  platform: 'node',
  sourcemap: !production,
  minify: production,
};

// Build extension
const extensionBuild = esbuild.build({
  ...commonOptions,
  entryPoints: ['src/extension.ts'],
  outfile: 'dist/extension.js',
});

// Build LSP server (separate bundle - runs as child process)
// Must bundle ALL dependencies since VSIX doesn't include node_modules
const serverBuild = esbuild.build({
  ...commonOptions,
  entryPoints: ['src/lsp/server.ts'],
  outfile: 'dist/lsp/server.js',
  external: [], // Bundle everything
  // Handle Node.js built-ins
  platform: 'node',
  target: 'node18',
});

// Run builds
Promise.all([extensionBuild, serverBuild])
  .then(() => {
    console.log('Build completed successfully');
    if (watch) {
      console.log('Watching for changes...');
    }
  })
  .catch(() => process.exit(1));
