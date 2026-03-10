import { describe, it, expect } from 'vitest';
import remarkS9References from './remarkS9References';
import type { Root, Text, Link } from 'mdast';

/** Helper: creates a minimal AST with a single text node. */
function makeTree(text: string): Root {
  return {
    type: 'root',
    children: [
      {
        type: 'paragraph',
        children: [{ type: 'text', value: text }],
      },
    ],
  };
}

/** Helper: gets the children of the first paragraph. */
function getInline(tree: Root) {
  const para = tree.children[0];
  return 'children' in para ? (para as { children: unknown[] }).children : [];
}

describe('remarkS9References', () => {
  const plugin = remarkS9References();

  it('converts #42 to ticket link', () => {
    const tree = makeTree('See #42 for details');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(3);
    expect((nodes[0] as Text).value).toBe('See ');
    expect((nodes[1] as Link).type).toBe('link');
    expect((nodes[1] as Link).url).toBe('/tickets/42');
    expect(((nodes[1] as Link).children[0] as Text).value).toBe('#42');
    expect((nodes[2] as Text).value).toBe(' for details');
  });

  it('converts #MAP-23 to ticket link', () => {
    const tree = makeTree('Fix #MAP-23');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(2);
    expect((nodes[1] as Link).url).toBe('/tickets/23');
    expect(((nodes[1] as Link).children[0] as Text).value).toBe('#MAP-23');
  });

  it('converts comment#3 to anchor link', () => {
    const tree = makeTree('See comment#3');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(2);
    expect((nodes[1] as Link).url).toBe('#comment-3');
    expect(((nodes[1] as Link).children[0] as Text).value).toBe('comment#3');
  });

  it('converts #42/comment#3 to ticket+comment link', () => {
    const tree = makeTree('Check #42/comment#3');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(2);
    expect((nodes[1] as Link).url).toBe('/tickets/42#comment-3');
    expect(((nodes[1] as Link).children[0] as Text).value).toBe('#42/comment#3');
  });

  it('converts #MAP-23/comment#1 to ticket+comment link', () => {
    const tree = makeTree('See #MAP-23/comment#1');
    plugin(tree);
    const nodes = getInline(tree);

    expect((nodes[1] as Link).url).toBe('/tickets/23#comment-1');
    expect(((nodes[1] as Link).children[0] as Text).value).toBe('#MAP-23/comment#1');
  });

  it('converts @alex to mention link', () => {
    const tree = makeTree('Hey @alex check this');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(3);
    expect((nodes[1] as Link).type).toBe('link');
    expect((nodes[1] as Link).url).toBe('mention:alex');
    expect(((nodes[1] as Link).children[0] as Text).value).toBe('@alex');
  });

  it('handles multiple references in one text node', () => {
    const tree = makeTree('#1 and #MAP-2 and @bob');
    plugin(tree);
    const nodes = getInline(tree);

    // #1, " and ", #MAP-2, " and ", @bob
    expect(nodes).toHaveLength(5);
    expect((nodes[0] as Link).url).toBe('/tickets/1');
    expect((nodes[2] as Link).url).toBe('/tickets/2');
    expect((nodes[4] as Link).url).toBe('mention:bob');
  });

  it('leaves plain text unchanged', () => {
    const tree = makeTree('No references here');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(1);
    expect((nodes[0] as Text).value).toBe('No references here');
  });

  it('handles text with only a reference', () => {
    const tree = makeTree('#99');
    plugin(tree);
    const nodes = getInline(tree);

    expect(nodes).toHaveLength(1);
    expect((nodes[0] as Link).url).toBe('/tickets/99');
  });
});
