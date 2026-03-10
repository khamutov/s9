import { useState, useRef, useEffect, useCallback } from 'react';
import styles from './FilterBar.module.css';

/** Autocomplete suggestion shown in the dropdown. */
interface Suggestion {
  /** Text to insert (e.g. "status:" or "status:new"). */
  insert: string;
  /** Left-column label in dropdown. */
  label: string;
  /** Right-column description. */
  hint: string;
}

/** Static filter key suggestions shown when input is empty or starts a new token. */
const FILTER_KEYS: Suggestion[] = [
  { insert: 'status:', label: 'status:', hint: 'new, in_progress, verify, done' },
  { insert: 'priority:', label: 'priority:', hint: 'P0–P5' },
  { insert: 'owner:', label: 'owner:', hint: 'user login' },
  { insert: 'component:', label: 'component:', hint: 'component path' },
  { insert: 'milestone:', label: 'milestone:', hint: 'milestone name' },
  { insert: 'type:', label: 'type:', hint: 'bug, feature' },
  { insert: 'is:', label: 'is:', hint: 'open, closed' },
  { insert: 'has:', label: 'has:', hint: 'estimation, milestone' },
  { insert: 'created:', label: 'created:', hint: '>2026-01-01, <2026-03-01' },
  { insert: 'updated:', label: 'updated:', hint: '>2026-01-01, <2026-03-01' },
];

/** Known values for specific filter keys. */
const FILTER_VALUES: Record<string, Suggestion[]> = {
  'status:': [
    { insert: 'status:new', label: 'new', hint: 'Newly created' },
    { insert: 'status:in_progress', label: 'in_progress', hint: 'Being worked on' },
    { insert: 'status:verify', label: 'verify', hint: 'Awaiting verification' },
    { insert: 'status:done', label: 'done', hint: 'Completed' },
  ],
  'priority:': [
    { insert: 'priority:P0', label: 'P0', hint: 'Critical' },
    { insert: 'priority:P1', label: 'P1', hint: 'High' },
    { insert: 'priority:P2', label: 'P2', hint: 'Medium' },
    { insert: 'priority:P3', label: 'P3', hint: 'Low' },
    { insert: 'priority:P4', label: 'P4', hint: 'Minor' },
    { insert: 'priority:P5', label: 'P5', hint: 'Trivial' },
  ],
  'type:': [
    { insert: 'type:bug', label: 'bug', hint: 'Bug report' },
    { insert: 'type:feature', label: 'feature', hint: 'Feature request' },
  ],
  'is:': [
    { insert: 'is:open', label: 'open', hint: 'Not done' },
    { insert: 'is:closed', label: 'closed', hint: 'Done' },
  ],
  'has:': [
    { insert: 'has:estimation', label: 'estimation', hint: 'Has time estimate' },
    { insert: 'has:milestone', label: 'milestone', hint: 'Assigned to milestone' },
  ],
};

interface FilterBarProps {
  /** Current filter query value. */
  value: string;
  /** Called when the user changes the filter text. */
  onChange: (value: string) => void;
  /** Placeholder text. */
  placeholder?: string;
}

/**
 * Filter input with autocomplete for the ticket micro-syntax.
 *
 * Shows filter key suggestions when starting a new token, and value
 * suggestions for known keys (status, priority, type, is, has).
 */
export default function FilterBar({
  value,
  onChange,
  placeholder = 'Filter tickets\u2026',
}: FilterBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [focused, setFocused] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);

  // Compute suggestions based on the current last token
  const suggestions = getSuggestions(value);
  const showDropdown = focused && suggestions.length > 0;

  // Reset active index when suggestions change
  useEffect(() => {
    setActiveIndex(-1);
  }, [suggestions.length]);

  // Global "/" shortcut to focus filter
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (
        e.key === '/' &&
        !e.ctrlKey &&
        !e.metaKey &&
        document.activeElement !== inputRef.current &&
        !(document.activeElement instanceof HTMLInputElement) &&
        !(document.activeElement instanceof HTMLTextAreaElement)
      ) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  const applySuggestion = useCallback(
    (suggestion: Suggestion) => {
      const tokens = value.split(/\s+/);
      // Replace the last (partial) token with the suggestion
      tokens[tokens.length - 1] = suggestion.insert;
      const newValue = tokens.join(' ');
      // If suggestion is a key (ends with ":"), don't add space
      onChange(suggestion.insert.endsWith(':') ? newValue : newValue + ' ');
      setActiveIndex(-1);
      inputRef.current?.focus();
    },
    [value, onChange],
  );

  function handleKeyDown(e: React.KeyboardEvent) {
    if (!showDropdown) {
      if (e.key === 'Escape') {
        inputRef.current?.blur();
      }
      return;
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setActiveIndex((i) => (i + 1) % suggestions.length);
        break;
      case 'ArrowUp':
        e.preventDefault();
        setActiveIndex((i) => (i <= 0 ? suggestions.length - 1 : i - 1));
        break;
      case 'Enter':
      case 'Tab':
        if (activeIndex >= 0) {
          e.preventDefault();
          applySuggestion(suggestions[activeIndex]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        inputRef.current?.blur();
        break;
    }
  }

  return (
    <div className={styles.filterBar}>
      <svg
        className={styles.icon}
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
        ref={inputRef}
        className={styles.input}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => {
          // Delay to allow click on dropdown item
          setTimeout(() => setFocused(false), 150);
        }}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        aria-label="Filter tickets"
        autoComplete="off"
        spellCheck={false}
      />
      <span className={styles.kbd}>/</span>
      {showDropdown && (
        <div className={styles.dropdown} role="listbox">
          {suggestions.map((s, i) => (
            <div
              key={s.insert}
              className={`${styles.dropdownItem} ${i === activeIndex ? styles.dropdownItemActive : ''}`}
              role="option"
              aria-selected={i === activeIndex}
              onMouseDown={(e) => {
                e.preventDefault();
                applySuggestion(s);
              }}
            >
              <span className={styles.dropdownKey}>{s.label}</span>
              <span className={styles.dropdownHint}>{s.hint}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Returns autocomplete suggestions for the current input value. */
function getSuggestions(value: string): Suggestion[] {
  const trimmed = value.trimEnd();
  // If empty or ends with a space, suggest all filter keys
  if (!trimmed || value.endsWith(' ')) {
    return FILTER_KEYS;
  }

  const tokens = trimmed.split(/\s+/);
  const lastToken = tokens[tokens.length - 1];

  // Check if the last token is a complete filter key (e.g. "status:")
  if (lastToken in FILTER_VALUES) {
    return FILTER_VALUES[lastToken];
  }

  // Check if the last token is a partial key:value (e.g. "status:n")
  const colonIdx = lastToken.indexOf(':');
  if (colonIdx > 0) {
    const key = lastToken.slice(0, colonIdx + 1);
    const partial = lastToken.slice(colonIdx + 1).toLowerCase();
    const values = FILTER_VALUES[key];
    if (values && partial) {
      return values.filter((s) => s.label.toLowerCase().startsWith(partial));
    }
    return [];
  }

  // Partial key match (e.g. "sta" → "status:")
  const lower = lastToken.toLowerCase();
  const matches = FILTER_KEYS.filter((s) =>
    s.label.toLowerCase().startsWith(lower),
  );
  return matches;
}
