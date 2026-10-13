import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {existsSync, readFileSync, readdirSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {createHash} from 'node:crypto';

assert.equal(process.platform, 'darwin', 'Run bundle verification on macOS');
const arch = process.argv[2];
assert.ok(['aarch64', 'x86_64'].includes(arch), 'Expected aarch64 or x86_64');
const bundle = resolve('src-tauri/target/release/bundle');
const app = join(bundle, 'macos/Whispera.app');
const run = (tool, args) => execFileSync(tool, args, {encoding: 'utf8'}).trim();
run('/usr/bin/codesign', ['--verify', '--deep', '--strict', '--verbose=2', app]);
const plist = JSON.parse(run('/usr/bin/plutil', ['-convert', 'json', '-o', '-', join(app, 'Contents/Info.plist')]));
assert.equal(plist.CFBundleIdentifier, 'app.whispera.desktop.preview');
assert.ok(plist.NSMicrophoneUsageDescription);
assert.ok(plist.NSScreenCaptureUsageDescription);
assert.equal(plist.LSUIElement, true);
const executable = join(app, 'Contents/MacOS', plist.CFBundleExecutable);
const binaryArch = arch === 'aarch64' ? 'arm64' : 'x86_64';
assert.ok(run('/usr/bin/lipo', ['-archs', executable]).split(' ').includes(binaryArch));
const ffmpeg = join(app, 'Contents/Resources/bin/ffmpeg');
assert.ok(existsSync(ffmpeg));
assert.ok(run('/usr/bin/lipo', ['-archs', ffmpeg]).split(' ').includes(binaryArch));
assert.match(run(ffmpeg, ['-version']), /ffmpeg version/);
for (const file of ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'bin/ffmpeg.LICENSE', 'vendor/LICENSE']) {
  assert.ok(existsSync(join(app, 'Contents/Resources', file)), `Missing ${file}`);
}
const archive = readdirSync(join(bundle, 'macos')).find(name => name.endsWith('.app.tar.gz'));
assert.ok(archive, 'Missing updater archive');
assert.ok(readFileSync(join(bundle, 'macos', `${archive}.sig`), 'utf8').trim().length > 30);
const dmg = readdirSync(join(bundle, 'dmg')).find(name => name.endsWith('.dmg'));
assert.ok(dmg, 'Missing DMG');
const files = [join(bundle, 'dmg', dmg), join(bundle, 'macos', archive)];
const report = {version: plist.CFBundleShortVersionString, arch, signature: 'ad-hoc', notarized: false,
  hardwareValidated: false, files: files.map(path => ({name: path.split('/').at(-1),
    sha256: createHash('sha256').update(readFileSync(path)).digest('hex')}))};
writeFileSync(join(bundle, 'macos/verification.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report, null, 2));
