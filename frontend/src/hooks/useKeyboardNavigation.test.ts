import { renderHook, act } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { useKeyboardNavigation } from './useKeyboardNavigation';

const wrapper = ({ children }: { children: React.ReactNode }) =>
  MemoryRouter({ children, initialEntries: ['/tickets'] });

function fireKey(key: string) {
  const event = new KeyboardEvent('keydown', { key, bubbles: true });
  document.dispatchEvent(event);
}

describe('useKeyboardNavigation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('starts with selectedIndex -1', () => {
    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 5,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );
    expect(result.current.selectedIndex).toBe(-1);
  });

  it('moves selection down with j key', () => {
    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 3,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );

    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(0);

    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(1);

    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(2);

    // Should not go past last item
    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(2);
  });

  it('moves selection up with k key', () => {
    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 3,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );

    // Move down first
    act(() => fireKey('j'));
    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(1);

    act(() => fireKey('k'));
    expect(result.current.selectedIndex).toBe(0);

    // Should not go below 0
    act(() => fireKey('k'));
    expect(result.current.selectedIndex).toBe(0);
  });

  it('supports ArrowDown and ArrowUp keys', () => {
    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 3,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );

    act(() => fireKey('ArrowDown'));
    expect(result.current.selectedIndex).toBe(0);

    act(() => fireKey('ArrowDown'));
    expect(result.current.selectedIndex).toBe(1);

    act(() => fireKey('ArrowUp'));
    expect(result.current.selectedIndex).toBe(0);
  });

  it('does not respond when disabled', () => {
    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 3,
          getItemHref: (i) => `/tickets/${i}`,
          enabled: false,
        }),
      { wrapper },
    );

    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(-1);
  });

  it('does not respond when input is focused', () => {
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();

    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 3,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );

    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(-1);

    document.body.removeChild(input);
  });

  it('does not respond when textarea is focused', () => {
    const textarea = document.createElement('textarea');
    document.body.appendChild(textarea);
    textarea.focus();

    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 3,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );

    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(-1);

    document.body.removeChild(textarea);
  });

  it('resets selection when itemCount changes', () => {
    const { result, rerender } = renderHook(
      ({ count }) =>
        useKeyboardNavigation({
          itemCount: count,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper, initialProps: { count: 5 } },
    );

    act(() => fireKey('j'));
    act(() => fireKey('j'));
    expect(result.current.selectedIndex).toBe(1);

    rerender({ count: 3 });
    expect(result.current.selectedIndex).toBe(-1);
  });

  it('allows programmatic setSelectedIndex', () => {
    const { result } = renderHook(
      () =>
        useKeyboardNavigation({
          itemCount: 5,
          getItemHref: (i) => `/tickets/${i}`,
        }),
      { wrapper },
    );

    act(() => result.current.setSelectedIndex(3));
    expect(result.current.selectedIndex).toBe(3);
  });
});
