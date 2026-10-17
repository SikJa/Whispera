import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {existsSync, readFileSync, readdirSync, writeFileSync, mkdtempSync, mkdirSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createHash} from 'node:crypto';

assert.equal(process.platform, 'darwin', 'Run bundle verification on macOS');
assert.equal(process.env.WHISPERA_SIGNATURE_VERIFIED, '1', 'Verify updater signatures before the bundle');
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
const startup=execFileSync(executable, ['--ci-smoke'], {encoding:'utf8', timeout:30000,
  env:{...process.env, WHISPERA_CI_SMOKE:'1'}});
assert.match(startup, /WHISPERA_CI_READY/, 'The bundled native app must start and exit cleanly on the macOS runner');
for (const file of ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'bin/ffmpeg.LICENSE', 'vendor/LICENSE']) {
  assert.ok(existsSync(join(app, 'Contents/Resources', file)), `Missing ${file}`);
}
const archive = readdirSync(join(bundle, 'macos')).find(name => name.endsWith('.app.tar.gz'));
assert.ok(archive, 'Missing updater archive');
assert.ok(readFileSync(join(bundle, 'macos', `${archive}.sig`), 'utf8').trim().length > 30);
const dmg = readdirSync(join(bundle, 'dmg')).find(name => name.endsWith('.dmg'));
assert.ok(dmg, 'Missing DMG');
const portable = join(bundle, 'macos', `Whispera_${plist.CFBundleShortVersionString}_${arch}.app.zip`);
run('/usr/bin/ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', app, portable]);
const temporary = mkdtempSync(join(tmpdir(), 'whispera-package-check-'));
let mounted = false;
const mount = join(temporary, 'volume');
function verifyExtracted(path) {
  run('/usr/bin/codesign', ['--verify', '--deep', '--strict', path]);
  const binary = join(path, 'Contents/MacOS', plist.CFBundleExecutable);
  assert.ok(run('/usr/bin/lipo', ['-archs', binary]).split(' ').includes(binaryArch));
  assert.match(execFileSync(binary, ['--ci-smoke'], {encoding:'utf8', timeout:30000,
    env:{...process.env, WHISPERA_CI_SMOKE:'1'}}), /WHISPERA_CI_READY/);
}
try {
  for (const directory of ['volume', 'Applications', 'portable', 'updater']) mkdirSync(join(temporary, directory));
  run('/usr/bin/hdiutil', ['verify', join(bundle, 'dmg', dmg)]);
  run('/usr/bin/hdiutil', ['attach', '-readonly', '-nobrowse', '-mountpoint', mount, join(bundle, 'dmg', dmg)]);
  mounted = true;
  const installed = join(temporary, 'Applications/Whispera.app');
  run('/usr/bin/ditto', [join(mount, 'Whispera.app'), installed]);
  verifyExtracted(installed);
  run('/usr/bin/ditto', ['-x', '-k', portable, join(temporary, 'portable')]);
  verifyExtracted(join(temporary, 'portable/Whispera.app'));
  run('/usr/bin/tar', ['-xzf', join(bundle, 'macos', archive), '-C', join(temporary, 'updater')]);
  verifyExtracted(join(temporary, 'updater/Whispera.app'));
} finally {
  if (mounted) run('/usr/bin/hdiutil', ['detach', mount]);
  // Only our mkdtemp directory is removed, after detaching the read-only image.
  rmSync(temporary, {recursive:true, force:true});
}
const files = [join(bundle, 'dmg', dmg), join(bundle, 'macos', archive), portable];
const report = {version: plist.CFBundleShortVersionString, arch, source: process.env.GITHUB_SHA,
  signature: 'ad-hoc', updaterSignatureVerified: true, startupVerified: true,
  dmgInstallVerified: true, portableVerified: true, updaterExtractionVerified: true, notarized: false,
  hardwareValidated: false, files: files.map(path => ({name: path.split('/').at(-1),
    sha256: createHash('sha256').update(readFileSync(path)).digest('hex')}))};
writeFileSync(join(bundle, 'macos/verification.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report, null, 2));
