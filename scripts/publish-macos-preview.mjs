import assert from 'node:assert/strict';
import {copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {basename, dirname, join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {updateMessage} from './publish-release.mjs';

const repo = 'SikJa/Whispera';
const digest = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const readJSON = path => JSON.parse(readFileSync(path, 'utf8'));
const gh = args => execFileSync(process.env.WHISPERA_GH || 'gh', args, {encoding: 'utf8'}).trim();

export function previewManifest(version, entries, notes) {
  assert.match(version, /^\d+\.\d+\.\d+-(?:beta|rc)\.\d+$/);
  assert.equal(entries.length, 2);
  const platforms = {};
  for (const entry of entries) {
    assert.ok(['aarch64', 'x86_64'].includes(entry.arch));
    assert.ok(!platforms[`darwin-${entry.arch}`], 'Duplicate architecture');
    assert.equal(entry.name, `Whispera_${version}_${entry.arch}.app.tar.gz`);
    assert.ok(entry.signature.trim().length > 30);
    platforms[`darwin-${entry.arch}`] = {signature: entry.signature.trim(),
      url: `https://github.com/${repo}/releases/download/v${version}/${entry.name}`};
  }
  return {version, notes:updateMessage(notes), pub_date: new Date().toISOString(), platforms};
}

function walk(directory) {
  return readdirSync(directory, {withFileTypes: true}).flatMap(entry => {
    if (entry.isSymbolicLink()) throw Error('Release artifacts cannot be symbolic links');
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

export function prepare(artifacts, source, notes) {
  assert.match(source, /^[a-f0-9]{40}$/);
  const version = readJSON('apps/desktop/src-tauri/tauri.conf.json').version;
  const reports = walk(resolve(artifacts)).filter(path => basename(path) === 'verification.json');
  assert.equal(reports.length, 2, 'Download exactly the two macOS build artifacts');
  const directory = resolve('.local/macos-release', version);
  mkdirSync(directory, {recursive: true});
  const entries = [], files = [];
  for (const reportPath of reports) {
    const report = readJSON(reportPath);
    assert.equal(report.version, version);
    assert.equal(report.source, source, 'Artifacts must come from this exact source commit');
    assert.equal(report.updaterSignatureVerified, true);
    assert.equal(report.startupVerified, true);
    assert.equal(report.notarized, false);
    assert.equal(report.hardwareValidated, false);
    assert.ok(['aarch64', 'x86_64'].includes(report.arch));
    assert.equal(report.files.length, 2);
    assert.equal(report.files.filter(file => file.name.endsWith('.dmg')).length, 1);
    assert.equal(report.files.filter(file => file.name.endsWith('.app.tar.gz')).length, 1);
    const paths = walk(dirname(dirname(reportPath)));
    for (const item of report.files) {
      const matches = paths.filter(path => basename(path) === item.name);
      assert.equal(matches.length, 1, `Missing or ambiguous artifact ${item.name}`);
      const original = matches[0];
      assert.equal(digest(original), item.sha256, 'Artifact checksum changed after CI verification');
      const extension = item.name.endsWith('.dmg') ? '.dmg' : '.app.tar.gz';
      assert.ok(item.name.endsWith(extension));
      const name = `Whispera_${version}_${report.arch}${extension}`;
      const destination = join(directory, name);
      copyFileSync(original, destination); files.push(destination);
      if (extension === '.app.tar.gz') {
        const signature = readFileSync(`${original}.sig`, 'utf8').trim();
        writeFileSync(`${destination}.sig`, signature + '\n'); files.push(`${destination}.sig`);
        entries.push({arch: report.arch, name, signature});
      }
    }
    const evidence = join(directory, `verification-${report.arch}.json`);
    copyFileSync(reportPath, evidence); files.push(evidence);
  }
  const manifest = previewManifest(version, entries, readFileSync('docs/update-message-macos.txt', 'utf8'));
  const latest = join(directory, 'latest.json');
  writeFileSync(latest, JSON.stringify(manifest, null, 2) + '\n'); files.push(latest);
  const checksums = join(directory, 'SHA256SUMS.txt');
  writeFileSync(checksums, files.map(path => `${digest(path)}  ${basename(path)}`).join('\n') + '\n'); files.push(checksums);
  const notesFile = join(directory, 'release-notes.md');
  writeFileSync(notesFile, notes + `\n\nSource: ${source}\n`);
  return {version, source, directory, files, latest, notesFile};
}

function release(tag) {
  const lines = gh(['api', `repos/${repo}/releases`, '--paginate', '--jq', `.[] | select(.tag_name == "${tag}") | @json`]);
  return lines ? JSON.parse(lines.split('\n')[0]) : undefined;
}
function verifyUploaded(value, files) {
  for (const file of files) {
    const bytes = readFileSync(file), asset = value.assets.find(asset => asset.name === basename(file));
    assert.ok(asset && asset.state === 'uploaded' && asset.size === bytes.length && asset.digest === `sha256:${digest(file)}`,
      `GitHub did not confirm the complete asset: ${basename(file)}`);
  }
}
export function publish(prepared) {
  const {version, source, files, latest, notesFile} = prepared, tag = `v${version}`;
  let value = release(tag);
  assert.ok(!value || value.draft, 'A published version is immutable; increment the preview version');
  if (!value) gh(['release', 'create', tag, '--repo', repo, '--target', source, '--draft', '--prerelease', '--latest=false',
    '--title', `Whispera ${version} - macOS preview`, '--notes-file', notesFile]);
  gh(['release', 'upload', tag, ...files, '--repo', repo, '--clobber']);
  verifyUploaded(release(tag), files);
  gh(['release', 'edit', tag, '--repo', repo, '--draft=false', '--prerelease', '--latest=false']);

  const channel = 'macos-preview';
  const channelNotes = `macOS preview update feed. Download the current DMGs and read the setup notes: [Whispera ${version}](https://github.com/${repo}/releases/tag/${tag}). Builds are ad-hoc signed, not Apple-notarized.`;
  value = release(channel);
  if (!value) gh(['release', 'create', channel, '--repo', repo, '--target', source, '--draft', '--prerelease', '--latest=false',
    '--title', 'macOS preview update channel', '--notes', channelNotes]);
  gh(['release', 'upload', channel, latest, '--repo', repo, '--clobber']);
  verifyUploaded(release(channel), [latest]);
  gh(['release', 'edit', channel, '--repo', repo, '--draft=false', '--prerelease', '--latest=false', '--notes', channelNotes]);
  console.log(`Published https://github.com/${repo}/releases/tag/${tag}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2);
  const option = name => args[args.indexOf(name) + 1];
  for (const name of ['--artifacts', '--source', '--notes']) assert.ok(args.includes(name) && option(name), `Missing ${name}`);
  const prepared = prepare(option('--artifacts'), option('--source'), readFileSync(option('--notes'), 'utf8'));
  if (args.includes('--publish')) publish(prepared);
  else console.log(`Prepared ${prepared.files.length} assets in ${prepared.directory}; no publication performed.`);
}
