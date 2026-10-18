import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveCommentReferences } from '../src/lib/comment-references.ts';
import { referenceContextFiles } from '../src/lib/comment-reference-context.ts';

const files = {
  'src/math.ts': 'export function double(value: number) {\n  return value * 2;\n}\n',
  'src/use.ts': 'import { double as twice } from "./math";\nexport const result = twice(4);\n',
  'src/other.ts': 'const double = (x: number) => x;\ndouble(1);\n',
};

test('finds a function from the comment line and resolves aliased calls', () => {
  const found = resolveCommentReferences(files, 'src/math.ts', 2);
  assert.equal(found.symbol, 'double');
  assert.ok(found.usages.some((usage) => usage.path === 'src/use.ts' && usage.text.includes('twice(4)')));
  assert.ok(!found.usages.some((usage) => usage.path === 'src/other.ts'));
});

test('cannot guess a function outside a named callable', () => {
  assert.throws(() => resolveCommentReferences({ 'src/a.ts': 'const answer = 1;' }, 'src/a.ts', 1), /named function/);
});

test('reference context keeps every usage or stops before TypeSafe', () => {
  const found = resolveCommentReferences(files, 'src/math.ts', 2);
  const context = referenceContextFiles(found, 'head', 64 * 1024);
  assert.ok(context.some((file) => file.path === 'src/use.ts'));
  assert.throws(() => referenceContextFiles(found, 'head', 30), /too large/);
});

test('resolves a configured path alias', () => {
  const found = resolveCommentReferences({
    ...files,
    'tsconfig.json': '{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}',
    'src/alias.ts': 'import { double as timesTwo } from "@/math";\ntimesTwo(3);\n',
  }, 'src/math.ts', 2);
  assert.ok(found.usages.some((usage) => usage.path === 'src/alias.ts' && usage.text.includes('timesTwo(3)')));
});

test('stops when a local import is not in the snapshot', () => {
  assert.throws(() => resolveCommentReferences({ ...files, 'src/missing.ts': 'import "./absent";' }, 'src/math.ts', 2), /cannot be resolved/);
});

test('resolves references through a re-export', () => {
  const found = resolveCommentReferences({
    ...files,
    'src/barrel.ts': 'export { double as twice } from "./math";\n',
    'src/barrel-use.ts': 'import { twice } from "./barrel";\ntwice(6);\n',
  }, 'src/math.ts', 2);
  assert.ok(found.usages.some((usage) => usage.path === 'src/barrel-use.ts' && usage.text.includes('twice(6)')));
});

test('stops for multiple project configs instead of using one for the whole branch', () => {
  assert.throws(() => resolveCommentReferences({ ...files, 'tsconfig.json': '{}', 'src/tsconfig.json': '{}' }, 'src/math.ts', 2), /Multiple TS\/JS configs/);
});

test('sends the selected diff with the definition and all reference files', () => {
  const found = resolveCommentReferences(files, 'src/math.ts', 2);
  const context = referenceContextFiles(found, 'head', 64 * 1024, '@@ -1,3 +1,3 @@\n+change');
  assert.equal(context.find((file) => file.path === 'src/math.ts')?.diff, '@@ -1,3 +1,3 @@\n+change');
  assert.ok(context.every((file) => file.ref === 'head'));
});
