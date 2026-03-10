/**
 * Custom remark plugin that transforms S9 micro-syntax references in text nodes
 * into link/span nodes in the MDAST.
 *
 * Supported patterns:
 * - `#MAP-23` → link to /tickets/23
 * - `#42` → link to /tickets/42
 * - `#42/comment#3` or `#MAP-23/comment#1` → link to /tickets/42#comment-3
 * - `comment#3` → anchor link to #comment-3
 * - `@alex` → mention span
 */

import type { Root, PhrasingContent, Text } from 'mdast';

/**
 * Regex matching all S9 micro-syntax references.
 * Order matters — longer patterns first for correct leftmost matching.
 *
 * Groups:
 * 1: slug-ticket/comment prefix (e.g. MAP)
 * 2: slug-ticket/comment ticket id
 * 3: slug-ticket/comment comment number
 * 4: numeric-ticket/comment ticket id
 * 5: numeric-ticket/comment comment number
 * 6: slug-ticket prefix (e.g. MAP)
 * 7: slug-ticket id
 * 8: numeric-ticket id
 * 9: standalone comment number
 * 10: mention login
 */
const REFERENCE_RE =
  /(?:#([A-Z][A-Z0-9]+)-(\d+)\/comment#(\d+))|(?:#(\d+)\/comment#(\d+))|(?:#([A-Z][A-Z0-9]+)-(\d+))|(?:#(\d+)\b)|(?:\bcomment#(\d+))|(?:@([a-zA-Z][\w.-]*))/g;

/** Splits text nodes and inserts link/span nodes for S9 references. */
export default function remarkS9References() {
  return (tree: Root) => {
    visit(tree);
  };
}

function visit(node: Root | PhrasingContent | { children?: unknown[] }) {
  if (!('children' in node) || !Array.isArray(node.children)) return;

  const newChildren: unknown[] = [];
  let changed = false;

  for (const child of node.children) {
    if ((child as { type: string }).type === 'text') {
      const parts = splitTextNode(child as Text);
      if (parts.length > 1 || parts[0] !== child) {
        newChildren.push(...parts);
        changed = true;
      } else {
        newChildren.push(child);
      }
    } else {
      visit(child as PhrasingContent);
      newChildren.push(child);
    }
  }

  if (changed) {
    node.children = newChildren;
  }
}

function splitTextNode(node: Text): PhrasingContent[] {
  const text = node.value;
  const results: PhrasingContent[] = [];
  let lastIndex = 0;

  REFERENCE_RE.lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = REFERENCE_RE.exec(text)) !== null) {
    // Add any text before this match
    if (match.index > lastIndex) {
      results.push({ type: 'text', value: text.slice(lastIndex, match.index) });
    }

    const fullMatch = match[0];

    if (match[1] !== undefined) {
      // Slug-ticket/comment: #MAP-23/comment#3
      const ticketId = match[2];
      const commentNum = match[3];
      results.push(makeLink(`/tickets/${ticketId}#comment-${commentNum}`, fullMatch));
    } else if (match[4] !== undefined) {
      // Numeric-ticket/comment: #42/comment#3
      const ticketId = match[4];
      const commentNum = match[5];
      results.push(makeLink(`/tickets/${ticketId}#comment-${commentNum}`, fullMatch));
    } else if (match[6] !== undefined) {
      // Slug-ticket: #MAP-23
      const ticketId = match[7];
      results.push(makeLink(`/tickets/${ticketId}`, fullMatch));
    } else if (match[8] !== undefined) {
      // Numeric ticket: #42
      const ticketId = match[8];
      results.push(makeLink(`/tickets/${ticketId}`, fullMatch));
    } else if (match[9] !== undefined) {
      // Standalone comment: comment#3
      const commentNum = match[9];
      results.push(makeLink(`#comment-${commentNum}`, fullMatch));
    } else if (match[10] !== undefined) {
      // Mention: @alex — encoded as link with mention: scheme
      results.push(makeLink(`mention:${match[10]}`, fullMatch));
    }

    lastIndex = match.index + fullMatch.length;
  }

  if (lastIndex === 0) return [node];

  if (lastIndex < text.length) {
    results.push({ type: 'text', value: text.slice(lastIndex) });
  }

  return results;
}

function makeLink(url: string, text: string): PhrasingContent {
  return {
    type: 'link',
    url,
    children: [{ type: 'text', value: text }],
  };
}
