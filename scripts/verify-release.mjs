import assert from 'node:assert/strict';
import {nextVersion,updateLockVersion} from './release-version.mjs';
import {manifest,updateMessage} from './publish-release.mjs';
assert.equal(nextVersion('0.2.15',[]),'0.2.15');
assert.equal(nextVersion('0.2.15',['v0.2.14']),'0.2.15');
assert.equal(nextVersion('0.2.15',['v0.2.15','v0.2.16','v0.2.100-beta','invalid']),'0.2.17');
assert.equal(nextVersion('0.2.15',['v1.0.0']),'1.0.1');
assert.equal(nextVersion('1.0.0',['v0.99.99']),'1.0.0');
assert.throws(()=>nextVersion('bad',[]));
for(const newline of ['\n','\r\n']){
  const lock=`name = "whispera-desktop"${newline}version = "0.2.23"${newline}`;
  assert.equal(updateLockVersion(lock,'0.2.23'),lock,'Preparing the same version is idempotent');
  assert.equal(updateLockVersion(lock,'0.2.24'),lock.replace('0.2.23','0.2.24'));
}
assert.throws(()=>updateLockVersion('name = "other"','0.2.23'));
const m=manifest('0.2.15','kazu00001/Whispera-K','signature','Cambios');
assert.equal(m.platforms['windows-x86_64'].url,'https://github.com/kazu00001/Whispera-K/releases/download/v0.2.15/Whispera_0.2.15_x64-setup.exe');
assert.equal(m.platforms['windows-x86_64'].signature,'signature');
assert.throws(()=>manifest('0.2.15','kazu00001/Whispera-K','',''));
assert.equal(updateMessage(' Un mensaje breve. '),'Un mensaje breve.');
assert.throws(()=>updateMessage(''));
assert.throws(()=>updateMessage('x'.repeat(281)));
assert.throws(()=>updateMessage('uno\ndos\ntres\ncuatro\ncinco'));
console.log('PASS: release version increments and complete updater manifests. No publication performed.');
