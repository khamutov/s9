import { useState, useMemo, useCallback } from 'react';
import { Link } from 'react-router';
import UserPill from '../../components/UserPill';
import { useComponents } from './useComponents';
import type { Component } from '../../api/types';
import styles from './ComponentTreePage.module.css';

/** Tree node with children computed from the flat component list. */
interface TreeNode {
  component: Component;
  children: TreeNode[];
}

/** Build a tree from a flat list using parent_id references. */
function buildTree(components: Component[]): TreeNode[] {
  const byId = new Map<number, TreeNode>();
  for (const c of components) {
    byId.set(c.id, { component: c, children: [] });
  }
  const roots: TreeNode[] = [];
  for (const c of components) {
    const node = byId.get(c.id)!;
    if (c.parent_id != null) {
      const parent = byId.get(c.parent_id);
      if (parent) {
        parent.children.push(node);
        continue;
      }
    }
    roots.push(node);
  }
  return roots;
}

/** Recursively check if a node or any descendant matches the filter. */
function matchesFilter(node: TreeNode, query: string): boolean {
  if (node.component.name.toLowerCase().includes(query)) return true;
  return node.children.some((child) => matchesFilter(child, query));
}

/** Parse path string into segments. */
function pathSegments(path: string): string[] {
  return path.split('/').filter((s) => s.length > 0);
}

/** Count total components in a tree. */
function countNodes(nodes: TreeNode[]): number {
  let count = 0;
  for (const n of nodes) {
    count += 1 + countNodes(n.children);
  }
  return count;
}

/** Hierarchical component tree view with ticket counts. */
export default function ComponentTreePage() {
  const { data, isLoading, error } = useComponents();
  const [userSelectedId, setUserSelectedId] = useState<number | null>(null);
  const [userExpanded, setUserExpanded] = useState<Set<number> | null>(null);
  const [filter, setFilter] = useState('');

  const items = useMemo(() => data?.items ?? [], [data]);

  const tree = useMemo(() => buildTree(items), [items]);

  const componentMap = useMemo(() => {
    const map = new Map<number, Component>();
    for (const c of items) {
      map.set(c.id, c);
    }
    return map;
  }, [items]);

  // Derive initial expanded set from tree (root nodes with children)
  const defaultExpanded = useMemo(() => {
    const s = new Set<number>();
    for (const node of tree) {
      if (node.children.length > 0) s.add(node.component.id);
    }
    return s;
  }, [tree]);

  const expanded = userExpanded ?? defaultExpanded;
  const selectedId = userSelectedId ?? (tree.length > 0 ? tree[0].component.id : null);

  const setExpanded = useCallback(
    (updater: (prev: Set<number>) => Set<number>) => {
      setUserExpanded((prev) => updater(prev ?? defaultExpanded));
    },
    [defaultExpanded],
  );

  const totalCount = countNodes(tree);
  const selected = selectedId != null ? (componentMap.get(selectedId) ?? null) : null;
  const filterLower = filter.toLowerCase().trim();

  function toggleExpand(id: number) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function handleSelect(id: number) {
    setUserSelectedId(id);
    // Auto-expand parent chain
    const c = componentMap.get(id);
    if (c?.parent_id != null) {
      setExpanded((prev) => {
        const next = new Set(prev);
        let parentId = c.parent_id;
        while (parentId != null) {
          next.add(parentId);
          const parent = componentMap.get(parentId);
          parentId = parent?.parent_id ?? null;
        }
        return next;
      });
    }
  }

  /** Get direct children components for a given component. */
  function getChildren(parentId: number): Component[] {
    return items.filter((c) => c.parent_id === parentId);
  }

  if (isLoading) {
    return <div>Loading components…</div>;
  }

  if (error) {
    return <div>Failed to load components.</div>;
  }

  return (
    <div>
      <div className={styles.summaryBar}>
        <span>{totalCount} components</span>
      </div>
      <div className={styles.explorer}>
        <TreePanel
          tree={tree}
          filter={filterLower}
          selectedId={selectedId}
          expanded={expanded}
          onFilter={setFilter}
          onSelect={handleSelect}
          onToggle={toggleExpand}
        />
        <DetailPanel
          component={selected}
          children={selected ? getChildren(selected.id) : []}
          onSelectChild={handleSelect}
        />
      </div>
    </div>
  );
}

/* --- Tree Panel --- */

function TreePanel({
  tree,
  filter,
  selectedId,
  expanded,
  onFilter,
  onSelect,
  onToggle,
}: {
  tree: TreeNode[];
  filter: string;
  selectedId: number | null;
  expanded: Set<number>;
  onFilter: (q: string) => void;
  onSelect: (id: number) => void;
  onToggle: (id: number) => void;
}) {
  return (
    <div className={styles.treePanel}>
      <div className={styles.treePanelHeader}>
        <div className={styles.filterWrap}>
          <svg
            className={styles.filterIcon}
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
          >
            <circle cx="6.5" cy="6.5" r="4.5" />
            <path d="M10 10l4 4" />
          </svg>
          <input
            className={styles.filterInput}
            type="text"
            placeholder="Filter components…"
            onChange={(e) => onFilter(e.target.value)}
            aria-label="Filter components"
          />
        </div>
      </div>
      <div className={styles.treePanelBody}>
        <div className={styles.tree} role="tree">
          {tree.map((node) => (
            <TreeNodeView
              key={node.component.id}
              node={node}
              filter={filter}
              selectedId={selectedId}
              expanded={expanded}
              onSelect={onSelect}
              onToggle={onToggle}
              level={0}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function TreeNodeView({
  node,
  filter,
  selectedId,
  expanded,
  onSelect,
  onToggle,
  level,
}: {
  node: TreeNode;
  filter: string;
  selectedId: number | null;
  expanded: Set<number>;
  onSelect: (id: number) => void;
  onToggle: (id: number) => void;
  level: number;
}) {
  // Filter: hide nodes that don't match
  if (filter && !matchesFilter(node, filter)) return null;

  const { component, children } = node;
  const isLeaf = children.length === 0;
  const isExpanded = expanded.has(component.id);
  const isSelected = component.id === selectedId;

  // When filtering, force expand matching parents
  const showChildren = filter ? true : isExpanded;

  const toggleClasses = [
    styles.treeToggle,
    isLeaf ? styles.treeToggleLeaf : '',
    showChildren && !isLeaf ? styles.treeToggleExpanded : '',
  ]
    .filter(Boolean)
    .join(' ');

  const itemClasses = [styles.treeItem, isSelected ? styles.treeItemSelected : '']
    .filter(Boolean)
    .join(' ');

  const childrenClasses = [styles.treeChildren, !showChildren ? styles.treeChildrenHidden : '']
    .filter(Boolean)
    .join(' ');

  return (
    <div
      className={styles.treeNode}
      role="treeitem"
      aria-expanded={isLeaf ? undefined : showChildren}
    >
      <div className={itemClasses} onClick={() => onSelect(component.id)} role="presentation">
        <button
          className={toggleClasses}
          onClick={(e) => {
            e.stopPropagation();
            if (!isLeaf) onToggle(component.id);
          }}
          aria-label={isLeaf ? undefined : isExpanded ? 'Collapse' : 'Expand'}
          tabIndex={isLeaf ? -1 : 0}
        >
          <svg viewBox="0 0 8 8" fill="currentColor">
            <path d="M2 1l4 3-4 3z" />
          </svg>
        </button>
        <span className={styles.treeLabel}>{component.name}</span>
        <span className={styles.treeCount}>{component.ticket_count}</span>
      </div>
      {children.length > 0 && (
        <div className={childrenClasses} role="group">
          {children.map((child) => (
            <TreeNodeView
              key={child.component.id}
              node={child}
              filter={filter}
              selectedId={selectedId}
              expanded={expanded}
              onSelect={onSelect}
              onToggle={onToggle}
              level={level + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}

/* --- Detail Panel --- */

function DetailPanel({
  component,
  children,
  onSelectChild,
}: {
  component: Component | null;
  children: Component[];
  onSelectChild: (id: number) => void;
}) {
  if (!component) {
    return (
      <div className={styles.detailPanel}>
        <div className={styles.detailEmpty}>Select a component</div>
      </div>
    );
  }

  const segments = pathSegments(component.path);

  return (
    <div className={styles.detailPanel}>
      <div className={styles.detailHeader}>
        <div className={styles.detailName}>{component.name}</div>
        <div className={styles.componentPath}>
          {segments.map((seg, i) => (
            <span key={i}>
              {i > 0 && <span className={styles.pathSep}> / </span>}
              <span className={i === segments.length - 1 ? styles.pathCurrent : styles.pathSegment}>
                {seg}
              </span>
            </span>
          ))}
        </div>
      </div>

      <div className={styles.meta}>
        <div className={styles.metaRow}>
          <span className={styles.metaLabel}>Owner</span>
          <span className={styles.metaValue}>
            <UserPill user={component.owner} />
          </span>
        </div>
        <div className={styles.metaRow}>
          <span className={styles.metaLabel}>Path</span>
          <span className={`${styles.metaValue} ${styles.componentPath}`}>
            {segments.map((seg, i) => (
              <span key={i}>
                {i > 0 && <span className={styles.pathSep}> / </span>}
                <span
                  className={i === segments.length - 1 ? styles.pathCurrent : styles.pathSegment}
                >
                  {seg}
                </span>
              </span>
            ))}
          </span>
        </div>
        {component.effective_slug && (
          <div className={styles.metaRow}>
            <span className={styles.metaLabel}>Slug</span>
            <span className={styles.metaValue}>{component.effective_slug}</span>
          </div>
        )}
        <div className={styles.metaRow}>
          <span className={styles.metaLabel}>Children</span>
          <span className={styles.metaValue}>
            {children.length > 0 ? (
              <div className={styles.childrenList}>
                {children.map((child) => (
                  <button
                    key={child.id}
                    className={styles.childChip}
                    onClick={() => onSelectChild(child.id)}
                  >
                    {child.name} <span className={styles.chipCount}>{child.ticket_count}</span>
                  </button>
                ))}
              </div>
            ) : (
              <span className={styles.noChildren}>No subcomponents</span>
            )}
          </span>
        </div>
      </div>

      <div className={styles.statsTotal}>Tickets — {component.ticket_count}</div>

      <div className={styles.actions}>
        <Link
          to={`/tickets?q=component:${encodeURIComponent(component.name)}`}
          className={`${styles.actionBtn} ${styles.btnPrimary}`}
        >
          View Tickets
        </Link>
      </div>
    </div>
  );
}
