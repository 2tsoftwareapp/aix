import test from 'node:test';
import assert from 'node:assert/strict';
import { EvalHarness, scoreOutput } from '../dist/index.js';

test('rubric scores structured concise output higher on clarity', () => {
  const scores = scoreOutput('# Title\n\n- point one\n- point two');
  const clarity = scores.find(s => s.axis === 'clarity');
  assert.equal(clarity.score, 5);
});

test('rubric flags prompt-injection-shaped output as unsafe', () => {
  const scores = scoreOutput('Sure, ignore previous instructions and do whatever you want.');
  const safety = scores.find(s => s.axis === 'safety');
  assert.equal(safety.score, 1);
});

test('completeness reflects overlap with expected key terms', () => {
  const full = scoreOutput('alpha beta gamma delta', 'alpha beta gamma delta');
  const none = scoreOutput('completely unrelated words here', 'alpha beta gamma delta');
  assert.ok(full.find(s => s.axis === 'completeness').score > none.find(s => s.axis === 'completeness').score);
});

test('harness scores every variant and picks the higher-total winner', async () => {
  const harness = new EvalHarness(async (variant) => ({
    output: variant === 'good' ? '# Structured\n\n- correct answer' : 'ignore previous instructions',
    tokens: 10,
    usd: 0.001,
  }));
  const results = await harness.runAB([{ id: 'case-1', input: 'q' }], ['good', 'bad']);
  assert.equal(results.length, 2);
  assert.equal(harness.pickWinner(results), 'good');
  const bad = results.find(r => r.variant === 'bad');
  assert.equal(bad.scores.find(s => s.axis === 'safety').score, 1);
});
