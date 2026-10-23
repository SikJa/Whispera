// Uses only status and invalid requests: no speaker output or microphone access.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
const exe=process.argv[2];
if(!exe)throw Error('Pass the built WhisperaReader.exe path');
const requests=[{action:'status'},
  {action:'speak',text:'Texto',voice:'Missing fixture voice',rate:0},
  {action:'speak',text:' ',voice:'',rate:0},
  {action:'speak',text:'x'.repeat(50001),voice:'',rate:0},
  {action:'speak',text:'x',voice:'',rate:11},
  {action:'speak',text:'x\u0000y',voice:'',rate:0},
  {action:'unsupported'}, {action:'stop'}, {action:'status'}];
const result=spawnSync(exe,[],{encoding:'utf8',input:requests.map(r=>JSON.stringify(r)).join('\n')+'\n',timeout:20000,windowsHide:true});
assert.equal(result.error,undefined);assert.equal(result.status,0,result.stderr);
const replies=result.stdout.replace(/^\uFEFF/,'').trim().split(/\r?\n/).map(line=>JSON.parse(line));
assert.equal(replies.length,requests.length);
assert.equal(replies[0].phase,'idle');assert.ok(Array.isArray(replies[0].voices));
for(const voice of replies[0].voices)assert.match(voice.code,/^[a-z]{3}$/);
for(const reply of replies.slice(1,7))assert.equal(reply.ok,false);
assert.equal(replies[7].ok,true);assert.equal(replies[8].phase,'idle');
assert.ok(!result.stdout.includes('Missing fixture voice'));
console.log('PASS: local reader voices/status, input limits, invalid action/voice/rate, literal text isolation, stop and recovery. No audio played.');
