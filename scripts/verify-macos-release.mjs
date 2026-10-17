import assert from 'node:assert/strict';
import {previewManifest, prepare} from './publish-macos-preview.mjs';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
const version = '0.3.0-beta.1';
const entries = ['aarch64', 'x86_64'].map(arch => ({arch, name: `Whispera_${version}_${arch}.app.tar.gz`, signature: 'fixture-signature-not-a-real-key'.repeat(3)}));
const result = previewManifest(version, entries, 'macOS preview');
assert.deepEqual(Object.keys(result.platforms).sort(), ['darwin-aarch64', 'darwin-x86_64']);
assert.equal(result.platforms['darwin-aarch64'].url, `https://github.com/SikJa/Whispera/releases/download/v${version}/Whispera_${version}_aarch64.app.tar.gz`);
assert.throws(() => previewManifest('0.3.0', entries, ''));
assert.throws(() => previewManifest(version, [entries[0]], ''));
assert.throws(() => previewManifest(version, [entries[0], entries[0]], ''));
assert.throws(() => previewManifest(version, [entries[0], {...entries[1], name: '../../unsafe.tar.gz'}], ''));
assert.throws(() => previewManifest(version, [entries[0], {...entries[1], signature: ''}], ''));
assert.throws(() => previewManifest(version, entries, 'x'.repeat(281)));
console.log('PASS: macOS preview manifest requires both architectures, signatures and versioned repository URLs. No publication performed.');
const previous=process.cwd(), fixture=mkdtempSync(join(tmpdir(),'whispera-release-fixture-'));
const source='a'.repeat(40);
try {
  process.chdir(fixture);
  mkdirSync('apps/desktop/src-tauri',{recursive:true});mkdirSync('docs');
  writeFileSync('apps/desktop/src-tauri/tauri.conf.json',JSON.stringify({version}));
  writeFileSync('docs/update-message-macos.txt','Test fixture, not a published release.');
  for(const arch of ['aarch64','x86_64']) {
    const root=join('artifacts',arch);mkdirSync(join(root,'macos'),{recursive:true});mkdirSync(join(root,'dmg'));
    const files=['.dmg','.app.tar.gz','.app.zip'].map(extension=>{
      const name=`Whispera_${version}_${arch}${extension}`, data=Buffer.from(`fixture ${arch} ${extension}`);
      const path=join(root,extension==='.dmg'?'dmg':'macos',name);writeFileSync(path,data);
      if(extension==='.app.tar.gz')writeFileSync(path+'.sig','fixture-signature-not-a-real-key'.repeat(3));
      return{name,sha256:createHash('sha256').update(data).digest('hex')};
    });
    writeFileSync(join(root,'macos/verification.json'),JSON.stringify({version,arch,source,files,
      startupVerified:true,updaterSignatureVerified:true,dmgInstallVerified:true,portableVerified:true,
      updaterExtractionVerified:true,notarized:false,hardwareValidated:false}));
  }
  const result=prepare('artifacts',source,'Fixture');
  assert.equal(result.files.length,12);assert.equal(result.files.filter(path=>path.endsWith('.app.zip')).length,2);
  const path='artifacts/x86_64/macos/verification.json', report=JSON.parse(readFileSync(path,'utf8'));
  for(const key of ['startupVerified','updaterSignatureVerified','dmgInstallVerified','portableVerified','updaterExtractionVerified']) {
    writeFileSync(path,JSON.stringify({...report,[key]:false}));
    assert.throws(()=>prepare('artifacts',source,'Fixture'),`Reject missing ${key}`);
  }
  writeFileSync(path,JSON.stringify({...report,source:'b'.repeat(40)}));
  assert.throws(()=>prepare('artifacts',source,'Fixture'),/exact source commit/);
  writeFileSync(path,JSON.stringify(report));
  writeFileSync(`artifacts/x86_64/macos/Whispera_${version}_x86_64.app.zip`,'tampered');
  assert.throws(()=>prepare('artifacts',source,'Fixture'),/checksum changed/);
  console.log('PASS: DMG/ZIP/updater preparation, exact source, required package checks and tamper rejection. Synthetic artifacts only.');
} finally {process.chdir(previous);rmSync(fixture,{recursive:true,force:true});}
