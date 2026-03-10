import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router';

/** Returns true if the active element is an input that should consume keystrokes. */
function isEditableActive(): boolean {
  const el = document.activeElement;
  if (!el) return false;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return true;
  if ((el as HTMLElement).isContentEditable) return true;
  // Select elements
  if (el instanceof HTMLSelectElement) return true;
  return false;
}

interface UseKeyboardNavigationOptions {
  /** Total number of items in the list. */
  itemCount: number;
  /** Returns the URL to navigate to when Enter is pressed on the item at index. */
  getItemHref: (index: number) => string;
  /** Whether the hook is enabled (e.g. only when data is loaded). */
  enabled?: boolean;
}

interface UseKeyboardNavigationResult {
  /** Currently selected row index, or -1 if none. */
  selectedIndex: number;
  /** Set the selected index programmatically. */
  setSelectedIndex: (index: number) => void;
}

/**
 * Keyboard navigation for list pages.
 *
 * Handles:
 * - `j` / `ArrowDown` — move selection down
 * - `k` / `ArrowUp` — move selection up
 * - `Enter` — navigate to selected item
 * - `c` — navigate to create ticket
 *
 * Shortcuts are disabled when focus is inside an input, textarea, or contenteditable.
 */
export function useKeyboardNavigation({
  itemCount,
  getItemHref,
  enabled = true,
}: UseKeyboardNavigationOptions): UseKeyboardNavigationResult {
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [prevItemCount, setPrevItemCount] = useState(itemCount);
  const navigate = useNavigate();

  // Reset selection when item count changes (render-phase sync)
  if (itemCount !== prevItemCount) {
    setPrevItemCount(itemCount);
    setSelectedIndex(-1);
  }

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (isEditableActive()) return;

      // Global shortcuts that work regardless of list state
      if (e.key === 'c') {
        e.preventDefault();
        navigate('/tickets/new');
        return;
      }

      if (!enabled) return;

      switch (e.key) {
        case 'j':
        case 'ArrowDown': {
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = prev < itemCount - 1 ? prev + 1 : prev;
            scrollRowIntoView(next);
            return next;
          });
          break;
        }
        case 'k':
        case 'ArrowUp': {
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = prev > 0 ? prev - 1 : 0;
            scrollRowIntoView(next);
            return next;
          });
          break;
        }
        case 'Enter': {
          // Use functional updater to read current index without a ref
          setSelectedIndex((idx) => {
            if (idx >= 0 && idx < itemCount) {
              e.preventDefault();
              navigate(getItemHref(idx));
            }
            return idx;
          });
          break;
        }
      }
    },
    [enabled, itemCount, getItemHref, navigate],
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  return { selectedIndex, setSelectedIndex };
}

/** Scrolls the table row at the given index into view. */
function scrollRowIntoView(index: number) {
  requestAnimationFrame(() => {
    const row = document.querySelectorAll('tbody tr')[index];
    if (row && typeof row.scrollIntoView === 'function') {
      row.scrollIntoView({ block: 'nearest' });
    }
  });
}
