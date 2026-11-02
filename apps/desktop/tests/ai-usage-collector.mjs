// Isolated Claude statusLine fixtures: no account, network, prompts or settings.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const exe=resolve(process.argv[2]);
const dir=mkdtempSync(join(tmpdir(),'whispera-quota-'));
const output=join(dir,'claude.json');
function run(input){return spawnSync(exe,[output],{encoding:'utf8',input:JSON.stringify(input),timeout:10000,windowsHide:true});}
try{
  const fixture={rate_limits:{five_hour:{used_percentage:21.5,resets_at:2000000000,secret:'do-not-retain'},seven_day:{used_percentage:81,resets_at:2000300000}},transcript_path:'private-transcript',session_id:'private-session',cwd:'private-directory',prompt:'private-prompt'};
  let result=run(fixture);assert.equal(result.status,0,result.stderr);
  const text=readFileSync(output,'utf8'),data=JSON.parse(text);
  assert.equal(data.schema,1);assert.ok(Math.abs(data.observedAt-Date.now()/1000)<60);
  assert.deepEqual(data.rate_limits,{five_hour:{used_percentage:21.5,resets_at:2000000000},seven_day:{used_percentage:81,resets_at:2000300000}});
  for(const privateValue of ['private-transcript','private-session','private-directory','private-prompt','do-not-retain'])assert.ok(!text.includes(privateValue)&&!result.stdout.includes(privateValue));
  assert.match(result.stdout,/78.5% libre/);
  result=run({rate_limits:{five_hour:{used_percentage:null},seven_day:{used_percentage:-1}}});assert.equal(result.status,0);assert.deepEqual(JSON.parse(readFileSync(output,'utf8')).rate_limits,{});
  result=run({});assert.equal(result.status,0);assert.deepEqual(JSON.parse(readFileSync(output,'utf8')).rate_limits,{});
  const before=readFileSync(output,'utf8');result=spawnSync(exe,[output],{encoding:'utf8',input:'not json',timeout:10000,windowsHide:true});assert.equal(result.status,1);assert.equal(readFileSync(output,'utf8'),before);
  result=run({rate_limits:{spend_limit:{used_percentage:50,used_usd:10,limit_usd:20,token:'private'}}});assert.equal(result.status,0);assert.deepEqual(JSON.parse(readFileSync(output,'utf8')).rate_limits.spend_limit,{used_percentage:50,used_usd:10,limit_usd:20});
  console.log('PASS: Claude quota collector sanitizes payload, preserves independent windows, handles missing/invalid values and rejects corrupt input without replacing the snapshot. No personal settings or network.');
}finally{rmSync(dir,{recursive:true,force:true});}
