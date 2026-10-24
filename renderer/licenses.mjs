// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,readdir,writeFile} from 'node:fs/promises';
const lock=JSON.parse(await readFile('package-lock.json','utf8'));
const parts=['MDcard renderer dependency notices\nVersions and integrity: package-lock.json. Bundled browser code and the local worker execute offline.\n'];
for(const [folder,entry] of Object.entries(lock.packages)){
 if(!folder||entry.dev)continue;
 let manifest;try{manifest=JSON.parse(await readFile(folder+'/package.json','utf8'));}catch{continue;}
 parts.push(`\n${'='.repeat(72)}\n${manifest.name} ${manifest.version}\nLicense: ${JSON.stringify(manifest.license||'see license files')}\nSource: ${JSON.stringify(manifest.repository||entry.resolved)}\nIntegrity: ${entry.integrity||''}\n`);
 const files=(await readdir(folder)).filter(n=>/^(?:licen[cs]e|copying|copyright|notice)/i.test(n));
 for(const file of files){try{parts.push(`\n--- ${file} ---\n`+await readFile(folder+'/'+file,'utf8'));}catch{}}
}
// mhchem embeds adapted MathJax/Apache-2.0 material below KaTeX's package root.
const mhchem=await readFile('node_modules/katex/contrib/mhchem/mhchem.js','utf8');
const start=mhchem.indexOf('/********');const end=mhchem.indexOf('*/',start);
if(start<0||end<start)throw Error('mhchem attribution header not found');
parts.push('\nKaTeX mhchem embedded upstream attribution and adaptation notice\nSource: https://github.com/KaTeX/KaTeX/blob/v0.19.0/contrib/mhchem/mhchem.js\n'+mhchem.slice(start,end+2)+'\n');
parts.push('\nPlantUML MIT build 1.2026.9\nSource: https://github.com/plantuml/plantuml/tree/v1.2026.9\nArtifact: https://github.com/plantuml/plantuml/releases/download/v1.2026.9/plantuml-mit-1.2026.9.jar\nSHA256: 835b2f2eb1d756da3f03bffdbd0baf252b4b858547a741ac8e972958a09c7f91\nThe MIT license applies to this explicitly selected MIT build. Other PlantUML distributions may have other licenses.\n');
await writeFile('THIRD_PARTY_NOTICES.txt',parts.join('')+'\n'+await readFile('PlantUML-MIT-LICENSE.txt','utf8')+'\nNode.js runtime 24.21.0\n'+await readFile('Node-LICENSE.txt','utf8'));
