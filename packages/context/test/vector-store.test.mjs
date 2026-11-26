import test from 'node:test';
import assert from 'node:assert/strict';
import { SimpleVectorStore } from '../dist/index.js';

test('vector store returns the closest document first and honors k', async () => {
  const store = new SimpleVectorStore();
  await store.index([
    { id: 'a', content: 'alpha', embedding: [1, 0, 0] },
    { id: 'b', content: 'beta', embedding: [0, 1, 0] },
    { id: 'c', content: 'gamma', embedding: [0.9, 0.1, 0] },
  ]);
  const hits = await store.query([1, 0, 0], 2);
  assert.equal(hits.length, 2);
  assert.equal(hits[0].file, 'a');
  assert.equal(hits[1].file, 'c');
  assert.ok(hits[0].score >= hits[1].score);
});

test('vector store roundtrips through JSON without losing documents', async () => {
  const store = new SimpleVectorStore();
  await store.index([{ id: 'a', content: 'alpha', embedding: [1, 2, 3] }]);
  const restored = SimpleVectorStore.fromJSON(store.toJSON());
  assert.equal(restored.size(), 1);
  const hits = await restored.query([1, 2, 3], 1);
  assert.equal(hits[0].file, 'a');
  assert.equal(hits[0].content, 'alpha');
  assert.ok(Math.abs(hits[0].score - 1) < 1e-9);
});

test('zero-magnitude embeddings score zero instead of NaN', async () => {
  const store = new SimpleVectorStore();
  await store.index([{ id: 'z', content: 'zero', embedding: [0, 0] }]);
  const hits = await store.query([0, 0], 1);
  assert.equal(hits[0].score, 0);
});
