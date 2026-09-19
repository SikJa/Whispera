import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
const files = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' }).split('\0').filter(Boolean);
const issues = [];
for (const file of files) {
  if (/(^|\/)(\.local|node_modules|target|test-results)(\/|$)|\.env($|\.)|\.(dmp|sqlite|pcm|lnk)$/i.test(file)) issues.push(file + ': forbidden file');
  if (/\.(png|ico|wav|woff2?)$/i.test(file)) continue;
  const text = readFileSync(file, 'utf8');
  if (/(?:gsk_|nvapi-|gh[pousr]_)[a-zA-Z0-9_-]{24,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(text)) issues.push(file + ': credential pattern');
  if (/C:[\\/]Users[\\/]Sikora|F:[\\/]Coding Proyects/i.test(text)) issues.push(file + ': private machine path');
}
if (!files.length) throw Error('No tracked files to scan');
if (issues.length) { console.error(issues.join('\n')); process.exit(1); }
console.log(`PASS: ${files.length} tracked files checked; no blocked data or credential patterns. Not a full secret audit.`);
