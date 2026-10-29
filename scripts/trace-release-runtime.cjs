// SPDX-License-Identifier: AGPL-3.0-only
// Build-only static dependency closure; platform-native Sharp files are added separately.
const fs = require('node:fs/promises');
const path = require('node:path');
const {nodeFileTrace} = require('./release-tools/node_modules/@vercel/nft');
(async () => {
  const base=path.resolve(process.argv[2]);
  const trace=await nodeFileTrace(['worker.mjs','export.mjs','node_modules/@marp-team/marp-cli/marp-cli.js'].map(p=>path.join(base,p)), {
    base,processCwd:base,exportsOnly:true,ignore:p=>/^(runtime|vendor)\//.test(p.replaceAll('\\','/')),
  });
  await fs.writeFile(process.argv[3],JSON.stringify({tool:'@vercel/nft',version:'1.11.0',files:[...trace.fileList].map(p=>p.replaceAll('\\','/')).sort(),warnings:[...trace.warnings].map(e=>e.message)},null,2)+'\n');
  console.log(`${trace.fileList.size} traced files; ${trace.warnings.size} warnings (recorded for review)`);
})();
