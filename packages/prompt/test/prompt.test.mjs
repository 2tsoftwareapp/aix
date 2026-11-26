import test from 'node:test';
import assert from 'node:assert/strict';
import { PromptAssembler, lintPrompt } from '../dist/index.js';
import { SkillRegistry } from '@x/registry';

function fakeSkill(id, roles) {
  return {
    id,
    dir: `/skills/${id}`,
    frontmatter: {
      name: id,
      description: `${id} description`,
      'x-kind': 'workflow',
      'x-tags': [],
      'x-version': '1.0.0',
      'x-roles': roles,
      'x-compatible': ['claude'],
    },
    body: `${id} body`,
    bodyTokenEstimate: 10,
    references: [],
    scripts: [],
    assets: [],
  };
}

function memoryWith(records) {
  return {
    search: async () => records,
    list: async () => records,
    save: async () => {},
  };
}

const PARTS = {
  base: 'You are a careful engineering assistant. Respond with output in the required format.',
  responseContract: 'Always respond with the requested artifact.',
  toneFormat: 'Be concise.',
};

test('assembler combines system parts and lists skills for the role', async () => {
  const registry = new SkillRegistry([fakeSkill('discussing-pro', ['architect']), fakeSkill('other-skill', ['reviewer'])]);
  const assembler = new PromptAssembler(registry, memoryWith([]), PARTS);
  const result = await assembler.assemble({ role: 'architect', phase: 'discuss', task: 'design a cache', tags: [] });
  assert.ok(result.system.includes('careful engineering assistant'));
  assert.ok(result.system.includes('discussing-pro'));
  assert.ok(!result.system.includes('other-skill'));
  assert.equal(result.user, 'design a cache');
  assert.ok(result.tokenEstimate > 0);
  assert.equal(result.skillMetadata.length, 1);
});

test('assembler only searches memory when tags are provided', async () => {
  const records = [{ id: 'm1', kind: 'lesson', body: 'remembered lesson', tags: ['cache'], createdAt: 'x' }];
  const registry = new SkillRegistry([]);
  const assembler = new PromptAssembler(registry, memoryWith(records), PARTS);
  const withoutTags = await assembler.assemble({ role: 'architect', phase: 'plan', task: 't', tags: [] });
  assert.equal(withoutTags.fewShot.length, 0);
  const withTags = await assembler.assemble({ role: 'architect', phase: 'plan', task: 't', tags: ['cache'] });
  assert.equal(withTags.fewShot.length, 1);
});

test('linter reports an error when no output constraint exists', () => {
  const issues = lintPrompt({ system: 'You are helpful.', skillMetadata: [{}], fewShot: [], user: 'x', tokenEstimate: 10 });
  assert.ok(issues.some(i => i.level === 'error' && i.message.includes('output')));
});

test('linter warns about contradictory output formats and missing skills', () => {
  const issues = lintPrompt({ system: 'respond with json. also respond in markdown. success criteria included.', skillMetadata: [], fewShot: [], user: 'x', tokenEstimate: 10 });
  assert.ok(issues.some(i => i.message.includes('contradiction')));
  assert.ok(issues.some(i => i.message.includes('No skills')));
});

test('linter passes a well-formed prompt without errors', () => {
  const issues = lintPrompt({ system: 'respond with json output. success criteria: correct.', skillMetadata: [{}], fewShot: [], user: 'x', tokenEstimate: 100 });
  assert.ok(!issues.some(i => i.level === 'error'));
});
