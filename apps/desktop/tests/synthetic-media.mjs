import {mkdirSync, existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFileSync} from 'node:child_process';
import ffmpeg from 'ffmpeg-static';

// Generated test patterns only. Never read the user's clipboard or recordings.
export function syntheticMedia() {
  const root = resolve('../../.local/synthetic-media');
  mkdirSync(root, {recursive:true});
  const run = (name, args) => {
    const path = resolve(root, name);
    if (!existsSync(path)) execFileSync(ffmpeg, ['-hide_banner','-loglevel','error','-y',...args,path], {windowsHide:true});
    return path;
  };
  return {
    '/fixture-1.png':run('one.png',['-f','lavfi','-i','testsrc2=size=336x336','-frames:v','1']),
    '/fixture-2.png':run('two.png',['-f','lavfi','-i','testsrc2=size=400x300','-frames:v','1']),
    '/poster.png':run('poster.png',['-f','lavfi','-i','testsrc2=size=320x180','-frames:v','1']),
    '/fixture.mp4':run('video.mp4',['-f','lavfi','-i','testsrc2=size=320x180:rate=10','-t','3','-an','-c:v','libx264','-pix_fmt','yuv420p'])
  };
}
