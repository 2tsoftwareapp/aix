import test from 'node:test';
import assert from 'node:assert/strict';
import { renderMermaid, renderImage } from '../dist/index.js';

test('renderMermaid escapes markup inside the diagram source', () => {
  const html = renderMermaid('graph TD; A-->B;\n</pre><script>alert(1)</script>');
  assert.ok(!html.includes('<script>alert(1)</script>'), 'raw script tag must not survive escaping');
  assert.ok(html.includes('&lt;script&gt;'), 'diagram markup must be entity-escaped');
  assert.ok(html.includes('graph TD; A--&gt;B;'));
});

test('renderMermaid preserves legitimate diagram content as text', () => {
  const html = renderMermaid('sequenceDiagram\nAlice->>Bob: Hello');
  assert.ok(html.includes('sequenceDiagram'));
  assert.ok(html.includes('Alice-&gt;&gt;Bob: Hello'));
});

test('renderImage produces a base64 svg data url', () => {
  const url = renderImage('<svg xmlns="http://www.w3.org/2000/svg"></svg>');
  assert.ok(url.startsWith('data:image/svg+xml;base64,'));
  const decoded = Buffer.from(url.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf-8');
  assert.equal(decoded, '<svg xmlns="http://www.w3.org/2000/svg"></svg>');
});
