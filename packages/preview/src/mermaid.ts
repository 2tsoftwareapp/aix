function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

export function renderMermaid(diagram: string): string {
  // The diagram source is embedded as text inside <pre>; without escaping, a
  // crafted diagram containing markup like </pre><script> would execute in
  // the preview page.
  const safeDiagram = escapeHtml(diagram);
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Mermaid Preview</title>
  <script src="https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js"></script>
  <style>
    body { margin: 2rem; font-family: system-ui, sans-serif; background: #fff; }
    .mermaid { max-width: 100%; }
  </style>
</head>
<body>
  <pre class="mermaid">${safeDiagram}</pre>
  <script>mermaid.initialize({ startOnLoad: true });</script>
</body>
</html>`;
}
