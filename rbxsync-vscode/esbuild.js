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
esbuild.build({
  ...commonOptions,
  entryPoints: ['src/extension.ts'],
  outfile: 'dist/extension.js',
})
  .then(() => {
    console.log('Build completed successfully');
    if (watch) {
      console.log('Watching for changes...');
    }
  })
  .catch(() => process.exit(1));
