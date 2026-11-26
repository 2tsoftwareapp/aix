interface FenceState {
  marker: "`" | "~";
  length: number;
}

function fenceAtLineStart(markdown: string, index: number): { marker: "`" | "~"; length: number; end: number } | null {
  let cursor = index;
  let spaces = 0;
  while (spaces < 4 && markdown[cursor] === " ") {
    spaces += 1;
    cursor += 1;
  }
  if (spaces > 3) return null;
  const marker = markdown[cursor];
  if (marker !== "`" && marker !== "~") return null;
  let length = 0;
  while (markdown[cursor + length] === marker) length += 1;
  if (length < 3) return null;
  return { marker, length, end: cursor + length };
}

/**
 * Removes HTML comments that are Markdown markup, while preserving comment-like
 * text inside inline code spans and fenced code blocks. Fences may be indented
 * by up to three spaces, matching CommonMark.
 */
export function stripMarkdownComments(markdown: string): string {
  let output = "";
  let index = 0;
  let lineStart = true;
  let fence: FenceState | null = null;
  let inlineCodeLength = 0;

  while (index < markdown.length) {
    if (lineStart) {
      const candidate = fenceAtLineStart(markdown, index);
      if (candidate && (inlineCodeLength === 0 || fence)) {
        if (!fence) {
          fence = { marker: candidate.marker, length: candidate.length };
        } else if (candidate.marker === fence.marker && candidate.length >= fence.length) {
          fence = null;
        }
        output += markdown.slice(index, candidate.end);
        index = candidate.end;
        lineStart = false;
        continue;
      }
    }

    if (!fence && markdown[index] === "`") {
      let length = 0;
      while (markdown[index + length] === "`") length += 1;
      output += markdown.slice(index, index + length);
      if (inlineCodeLength === 0) inlineCodeLength = length;
      else if (length === inlineCodeLength) inlineCodeLength = 0;
      index += length;
      lineStart = false;
      continue;
    }

    if (!fence && inlineCodeLength === 0 && markdown.startsWith("<!--", index)) {
      const end = markdown.indexOf("-->", index + 4);
      index = end === -1 ? markdown.length : end + 3;
      continue;
    }

    const char = markdown[index];
    output += char;
    lineStart = char === "\n";
    index += 1;
  }

  return output.replace(/^(?:[ \t]*\n)+/, "");
}
