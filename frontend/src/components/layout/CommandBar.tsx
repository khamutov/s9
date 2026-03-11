import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router';
import { listTickets } from '../../api/tickets';
import type { Ticket } from '../../api/types';
import styles from './CommandBar.module.css';

const DEBOUNCE_MS = 250;
const MAX_RESULTS = 8;

/** Global search / command bar with Cmd+K shortcut and quick-jump ticket search. */
export default function CommandBar() {
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Ticket[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [isLoading, setIsLoading] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();
  const containerRef = useRef<HTMLDivElement>(null);

  const closeDropdown = useCallback(() => {
    setIsOpen(false);
    setActiveIndex(-1);
  }, []);

  const jumpToTicket = useCallback(
    (ticket: Ticket) => {
      navigate(`/tickets/${ticket.id}`);
      setQuery('');
      setResults([]);
      closeDropdown();
      inputRef.current?.blur();
    },
    [navigate, closeDropdown],
  );

  // Debounced search
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);

    const trimmed = query.trim();
    if (!trimmed) {
      setResults([]);
      setIsOpen(false);
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const data = await listTickets({ q: trimmed, page_size: MAX_RESULTS });
        setResults(data.items);
        setIsOpen(true);
        setActiveIndex(-1);
      } catch {
        setResults([]);
      } finally {
        setIsLoading(false);
      }
    }, DEBOUNCE_MS);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query]);

  // Global keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    }
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        closeDropdown();
      }
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [closeDropdown]);

  function handleInputKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Escape') {
      if (isOpen) {
        closeDropdown();
      } else {
        inputRef.current?.blur();
      }
      return;
    }
    if (!isOpen || results.length === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((prev) => (prev < results.length - 1 ? prev + 1 : 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((prev) => (prev > 0 ? prev - 1 : results.length - 1));
    } else if (e.key === 'Enter' && activeIndex >= 0) {
      e.preventDefault();
      jumpToTicket(results[activeIndex]);
    }
  }

  return (
    <div className={styles.commandBar} ref={containerRef}>
      <svg
        className={styles.icon}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        strokeLinecap="round"
      >
        <circle cx="6.5" cy="6.5" r="4.5" />
        <path d="M10 10l4 4" />
      </svg>
      <input
        ref={inputRef}
        className={styles.input}
        type="text"
        placeholder="Search or jump to..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={handleInputKeyDown}
        onFocus={() => {
          if (results.length > 0) setIsOpen(true);
        }}
        role="combobox"
        aria-expanded={isOpen}
        aria-autocomplete="list"
        aria-controls="command-bar-results"
        aria-activedescendant={activeIndex >= 0 ? `cb-result-${activeIndex}` : undefined}
      />
      <kbd className={styles.kbd}>⌘K</kbd>

      {isOpen && (
        <ul className={styles.dropdown} id="command-bar-results" role="listbox">
          {isLoading && results.length === 0 ? (
            <li className={styles.dropdownHint}>Searching…</li>
          ) : results.length === 0 ? (
            <li className={styles.dropdownHint}>No tickets found</li>
          ) : (
            results.map((ticket, i) => (
              <li
                key={ticket.id}
                id={`cb-result-${i}`}
                role="option"
                aria-selected={i === activeIndex}
                className={`${styles.resultItem} ${i === activeIndex ? styles.resultItemActive : ''}`}
                onMouseEnter={() => setActiveIndex(i)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  jumpToTicket(ticket);
                }}
              >
                <span className={styles.resultId}>{ticket.slug ?? `#${ticket.id}`}</span>
                <span className={styles.resultTitle}>{ticket.title}</span>
                <span className={styles.resultStatus}>{ticket.status.replace('_', ' ')}</span>
              </li>
            ))
          )}
        </ul>
      )}
    </div>
  );
}
