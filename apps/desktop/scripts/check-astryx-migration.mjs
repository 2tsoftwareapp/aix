import {readFileSync, readdirSync, statSync} from 'node:fs';
import {join} from 'node:path';

const root = new URL('../src', import.meta.url).pathname;
const files = [];
function walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.(ts|tsx|css)$/.test(path)) files.push(path);
  }
}
walk(root);

const rules = [
  {pattern: /from\s+["'][^"']*components\/ui|from\s+["']\.\/ui\//, label: 'legacy shadcn/ui import'},
  {pattern: /from\s+["']sonner["']/, label: 'sonner import'},
  {pattern: /from\s+["']react-markdown["']/, label: 'react-markdown import'},
  {pattern: /from\s+["']react-resizable-panels["']/, label: 'react-resizable-panels import'},
  {pattern: /@tailwind\s+(base|components|utilities)/, label: 'Tailwind directive'},
  {pattern: /hsl\(var\(--/, label: 'legacy shadcn HSL token'},
];

const failures = [];
for (const file of files) {
  const text = readFileSync(file, 'utf8');
  for (const rule of rules) {
    if (rule.pattern.test(text)) failures.push(`${file}: ${rule.label}`);
  }
}

if (failures.length > 0) {
  console.error('Astryx migration guard failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(`Astryx migration guard passed (${files.length} files scanned).`);
