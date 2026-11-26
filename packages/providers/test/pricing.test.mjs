import test from 'node:test';
import assert from 'node:assert/strict';
import { PRICING_VERSION, PROVIDER_PRICING, estimateUsd } from '../dist/index.js';

test('pricing table carries a review version stamp and positive rates', () => {
  assert.match(PRICING_VERSION, /^\d{4}-\d{2}$/);
  for (const price of Object.values(PROVIDER_PRICING)) {
    assert.ok(price.inputPerMillion > 0);
    assert.ok(price.outputPerMillion > 0);
  }
});

test('estimateUsd scales linearly per million tokens', () => {
  const usd = estimateUsd({ inputPerMillion: 3, outputPerMillion: 15 }, 1_000_000, 1_000_000);
  assert.equal(usd, 18);
  assert.equal(estimateUsd({ inputPerMillion: 3, outputPerMillion: 15 }, 0, 0), 0);
});
