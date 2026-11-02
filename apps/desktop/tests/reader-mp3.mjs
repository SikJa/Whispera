// Real synthesis and MP3 decode in a temporary directory. Never plays audio.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,readFileSync,existsSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const [reader,ffmpeg]=process.argv.slice(2);
if(!reader||!ffmpeg)throw Error('Pass WhisperaReader.exe and ffmpeg.exe paths');
function execute(binary,args,input){
 const result=spawnSync(binary,args,{input,encoding:'utf8',windowsHide:true,timeout:30000});
 assert.equal(result.error,undefined);return result;
}
const directory=mkdtempSync(join(tmpdir(),"whispera-mp3-á '$-"));
try {
 const status=execute(reader,[],JSON.stringify({action:'status'})+'\n');
 assert.equal(status.status,0);
 const voice=JSON.parse(status.stdout.replace(/^\uFEFF/,'').trim()).voices[0]?.id;
 assert.ok(voice,'A Windows desktop voice is required for silent export verification');
 const wave=join(directory,'speech.wav'),mp3=join(directory,'speech.mp3');
 const request={action:'render-wave',text:'Prueba local de exportación. <speak> Texto literal & sin servicios externos.',voice,rate:1,path:wave};
 const rendered=execute(reader,['--render-wave'],JSON.stringify(request)+'\n');
 assert.equal(rendered.status,0,rendered.stdout);assert.equal(JSON.parse(rendered.stdout.replace(/^\uFEFF/,'').trim()).ok,true);
 const bytes=readFileSync(wave);assert.equal(bytes.toString('ascii',0,4),'RIFF');assert.equal(bytes.toString('ascii',8,12),'WAVE');assert.ok(bytes.length>1000);
 const encoded=execute(ffmpeg,['-hide_banner','-loglevel','error','-nostdin','-n','-i',wave,'-map','0:a:0','-vn','-map_metadata','-1','-c:a','libmp3lame','-b:a','128k','-f','mp3',mp3]);
 assert.equal(encoded.status,0);assert.ok(readFileSync(mp3).length>1000);
 assert.equal(execute(ffmpeg,['-hide_banner','-loglevel','error','-nostdin','-i',mp3,'-f','null','-']).status,0);
 const invalid=join(directory,'invalid.wav');
 assert.notEqual(execute(reader,['--render-wave'],JSON.stringify({...request,voice:'Missing voice fixture',path:invalid})+'\n').status,0);
 assert.equal(existsSync(invalid),false);
 // A live playback session cannot be instructed to write arbitrary files.
 const refused=execute(reader,[],JSON.stringify({...request,path:invalid})+'\n');
 assert.equal(JSON.parse(refused.stdout.replace(/^\uFEFF/,'').trim()).ok,false);assert.equal(existsSync(invalid),false);
 console.log('PASS: silent Windows synthesis with chosen voice/rate and literal text; valid WAV → decodable MP3; special-character paths; invalid voice and export mode isolation. No audio played.');
} finally {rmSync(directory,{recursive:true,force:true});}
