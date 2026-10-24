// Maintenance-only dependency tracer; never included in the application archive.
const fs = require('node:fs/promises');
const path = require('node:path');
function option(name, fallback) { const i=process.argv.indexOf(name); return i<0?fallback:process.argv[i+1]; }
const toolRoot=path.resolve(option('--tool-root', path.join(__dirname,'../node_modules/@vercel/nft')));
const { nodeFileTrace } = require(toolRoot);
(async () => {
  const root = path.resolve(__dirname, '..');
  const base = path.resolve(option('--renderer-root', path.join(root, 'renderer')));
  const inputs = ['worker.mjs', 'export.mjs', 'node_modules/@marp-team/marp-cli/marp-cli.js'];
  const trace = await nodeFileTrace(inputs.map(p => path.join(base, p)), {
    base, processCwd: base, exportsOnly: true,
    ignore: p => /^(runtime|vendor)\//.test(p.replaceAll('\\', '/')),
  });
  const result = {
    tool: '@vercel/nft', version: require(path.join(toolRoot,'package.json')).version,
    source: 'https://github.com/vercel/nft', inputs,
    files: [...trace.fileList].sort().map(p => ({ path: p.replaceAll('\\', '/'), ...trace.reasons.get(p), parents: [...(trace.reasons.get(p)?.parents || [])].map(p => p.replaceAll('\\', '/')) })),
    warnings: [...trace.warnings].map(e => e.message),
  };
  await fs.writeFile(path.resolve(option('--output',path.join(root,'dependency-trace.json'))), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ files: result.files.length, warnings: result.warnings.length }));
})();
