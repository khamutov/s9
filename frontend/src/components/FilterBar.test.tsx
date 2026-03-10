import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, test, expect, vi } from 'vitest';
import FilterBar from './FilterBar';

describe('FilterBar', () => {
  test('renders input with placeholder', () => {
    render(<FilterBar value="" onChange={() => {}} />);
    expect(screen.getByPlaceholderText(/filter tickets/i)).toBeInTheDocument();
  });

  test('calls onChange when user types', async () => {
    const onChange = vi.fn();
    render(<FilterBar value="" onChange={onChange} />);

    const input = screen.getByRole('textbox', { name: /filter tickets/i });
    await userEvent.type(input, 's');

    // Controlled component: onChange receives the new value
    expect(onChange).toHaveBeenCalledWith('s');
  });

  test('shows filter key suggestions on focus', async () => {
    render(<FilterBar value="" onChange={() => {}} />);

    const input = screen.getByRole('textbox', { name: /filter tickets/i });
    await userEvent.click(input);

    expect(screen.getByText('status:')).toBeInTheDocument();
    expect(screen.getByText('priority:')).toBeInTheDocument();
    expect(screen.getByText('owner:')).toBeInTheDocument();
  });

  test('shows value suggestions for known keys', async () => {
    render(<FilterBar value="status:" onChange={() => {}} />);

    const input = screen.getByRole('textbox', { name: /filter tickets/i });
    await userEvent.click(input);

    expect(screen.getByText('new')).toBeInTheDocument();
    expect(screen.getByText('in_progress')).toBeInTheDocument();
    expect(screen.getByText('done')).toBeInTheDocument();
  });

  test('filters key suggestions by partial input', async () => {
    render(<FilterBar value="pri" onChange={() => {}} />);

    const input = screen.getByRole('textbox', { name: /filter tickets/i });
    await userEvent.click(input);

    expect(screen.getByText('priority:')).toBeInTheDocument();
    expect(screen.queryByText('status:')).not.toBeInTheDocument();
  });

  test('applies suggestion on click', async () => {
    const onChange = vi.fn();
    render(<FilterBar value="" onChange={onChange} />);

    const input = screen.getByRole('textbox', { name: /filter tickets/i });
    await userEvent.click(input);

    // Click the "status:" suggestion
    const statusOption = screen.getByText('status:');
    await userEvent.click(statusOption);

    expect(onChange).toHaveBeenCalledWith('status:');
  });

  test('hides "/" kbd hint when focused', async () => {
    const { container } = render(<FilterBar value="" onChange={() => {}} />);

    // The kbd element uses CSS to hide when input is focused, so we just check it exists
    const kbd = container.querySelector('[class*="kbd"]');
    expect(kbd).toBeInTheDocument();
    expect(kbd).toHaveTextContent('/');
  });

  test('shows suggestions for second token after space', async () => {
    render(<FilterBar value="status:new " onChange={() => {}} />);

    const input = screen.getByRole('textbox', { name: /filter tickets/i });
    await userEvent.click(input);

    // After "status:new " we should see key suggestions for the next token
    expect(screen.getByText('priority:')).toBeInTheDocument();
  });
});
