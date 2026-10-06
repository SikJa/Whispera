import {readFileSync,writeFileSync,existsSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
export function manifest(version,repo,signature,notes){
  if(!/^\d+\.\d+\.\d+$/.test(version)||!/^[-\w]+\/[-\w]+$/.test(repo)||!signature.trim())throw Error('Datos de actualización incompletos');
  return {version,notes,pub_date:new Date().toISOString(),platforms:{'windows-x86_64':{signature:signature.trim(),url:`https://github.com/${repo}/releases/download/v${version}/Whispera_${version}_x64-setup.exe`}}};
}
export function publish(){
  const gh=process.env.WHISPERA_GH||'gh';
  const info=JSON.parse(readFileSync('.local/release.json','utf8'));
  const {version,sha,repo}=info,tag=`v${version}`;
  const dir='apps/desktop/src-tauri/target/release/bundle/nsis';
  const name=`Whispera_${version}_x64-setup.exe`,exe=`${dir}/${name}`,sig=exe+'.sig';
  if(!existsSync(exe)||!existsSync(sig))throw Error('Falta el instalador firmado');
  const notes=readFileSync('.local/release-notes.md','utf8');
  const latest=`${dir}/latest.json`,checksum=exe+'.sha256';
  writeFileSync(latest,JSON.stringify(manifest(version,repo,readFileSync(sig,'utf8'),notes),null,2)+'\n');
  const hash=createHash('sha256').update(readFileSync(exe)).digest('hex');
  writeFileSync(checksum,`${hash}  ${name}\n`);
  // Keep a stable, direct download URL for the README across release versions.
  const directName='Whispera-K-setup.exe',direct=`${dir}/${directName}`;
  copyFileSync(exe,direct);
  copyFileSync(sig,direct+'.sig');
  writeFileSync(direct+'.sha256',`${hash}  ${directName}\n`);
  const files=[exe,sig,checksum,latest,direct,direct+'.sig',direct+'.sha256'];
  const findRelease=()=>{
    // The by-tag API returns 404 for a draft whose Git tag is not created yet.
    const value=execFileSync(gh,['api',`repos/${repo}/releases`,'--paginate','--jq',`.[] | select(.tag_name == "${tag}") | @json`],{encoding:'utf8'}).trim();
    return value?JSON.parse(value.split('\n')[0]):undefined;
  };
  let release=findRelease();
  if(release&&!release.draft)throw Error('Esta versión ya está publicada; no se reemplaza un instalador anunciado');
  if(!release)execFileSync(gh,['release','create',tag,'--repo',repo,'--target',sha,'--draft','--title',`Whispera (K) ${version}`,'--notes-file','.local/release-notes.md'],{stdio:'inherit'});
  const pending=files.filter(file=>{
    const asset=release?.assets.find(a=>a.name===file.split('/').at(-1));
    return !asset||asset.digest!=='sha256:'+createHash('sha256').update(readFileSync(file)).digest('hex');
  });
  if(pending.length)execFileSync(gh,['release','upload',tag,...pending,'--repo',repo,'--clobber'],{stdio:'inherit'});
  release=findRelease();
  for(const file of files){
    const asset=release.assets.find(a=>a.name===file.split('/').at(-1));
    const data=readFileSync(file),digest='sha256:'+createHash('sha256').update(data).digest('hex');
    if(!asset||asset.state!=='uploaded'||asset.size!==data.length||asset.digest!==digest)throw Error('GitHub no confirmó el archivo completo: '+file);
  }
  execFileSync(gh,['release','edit',tag,'--repo',repo,'--draft=false','--latest'],{stdio:'inherit'});
  console.log(`Publicada y verificable desde Whispera: https://github.com/${repo}/releases/tag/${tag}`);
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href)publish();
