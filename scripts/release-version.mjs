import {readFileSync,writeFileSync,appendFileSync,mkdirSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {updateMessage} from './publish-release.mjs';

export function nextVersion(base,tags){
  const parse=v=>{const m=/^v?(\d+)\.(\d+)\.(\d+)$/.exec(v);return m?m.slice(1).map(Number):null;};
  const initial=parse(base);if(!initial)throw Error('La versión base debe ser estable: x.y.z');
  const compare=(a,b)=>a[0]-b[0]||a[1]-b[1]||a[2]-b[2];
  const latest=tags.map(parse).filter(Boolean).sort(compare).at(-1);
  if(!latest||compare(initial,latest)>0)return initial.join('.');
  return [latest[0],latest[1],latest[2]+1].join('.');
}
export function setVersion(version){
  const config='apps/desktop/src-tauri/tauri.conf.json';
  const c=JSON.parse(readFileSync(config,'utf8'));c.version=version;writeFileSync(config,JSON.stringify(c,null,2)+'\n');
  const manifest='apps/desktop/src-tauri/Cargo.toml';
  writeFileSync(manifest,readFileSync(manifest,'utf8').replace(/^(version\s*=\s*")[^"]+("\s*)$/m,`$1${version}$2`));
  const lock='apps/desktop/src-tauri/Cargo.lock';
  const original=readFileSync(lock,'utf8');
  const changed=original.replace(/(name = "whispera-desktop"\r?\nversion = ")[^"]+"/,`$1${version}"`);
  if(changed===original&&!original.includes(`name = "whispera-desktop"\nversion = "${version}"`))throw Error('No se encontró el paquete en Cargo.lock');
  writeFileSync(lock,changed);
}
export function prepare(){
  const repo=process.env.GITHUB_REPOSITORY||'SikJa/Whispera';
  const sha=process.env.GITHUB_SHA||execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
  const lines=execFileSync('gh',['api',`repos/${repo}/releases`,'--paginate','--jq','.[] | {tag: .tag_name, prerelease, draft, body} | @json'],{encoding:'utf8'}).trim();
  // --jq compact JSON objects are one object per line.
  const releases=lines?lines.split('\n').map(line=>JSON.parse(line)):[];
  const marker=`Source: ${sha}`;
  const existing=releases.find(r=>!r.prerelease&&!r.draft&&r.body?.includes(marker));
  if(existing){if(process.env.GITHUB_OUTPUT)appendFileSync(process.env.GITHUB_OUTPUT,'skip=true\n');return;}
  const base=JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json','utf8')).version;
  const stable=releases.filter(r=>!r.prerelease).map(r=>r.tag);
  const version=nextVersion(base,stable);
  setVersion(version);mkdirSync('.local',{recursive:true});
  const changes=execFileSync('git',['log','-n','8','--pretty=format:- %s'],{encoding:'utf8'}).trim();
  const notes=`# Whispera ${version}\n\nActualización publicada automáticamente después de compilar y pasar las pruebas.\n\n${changes}\n\n${marker}\n`;
  writeFileSync('.local/release-notes.md',notes);
  writeFileSync('.local/update-message.txt',updateMessage(readFileSync('docs/update-message.txt','utf8'))+'\n');
  writeFileSync('.local/release.json',JSON.stringify({version,sha,repo}));
  if(process.env.GITHUB_OUTPUT)appendFileSync(process.env.GITHUB_OUTPUT,`skip=false\nversion=${version}\n`);
  console.log(`Preparada versión ${version}`);
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href)prepare();
