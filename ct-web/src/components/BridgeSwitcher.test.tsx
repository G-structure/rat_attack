import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, test, vi } from 'vitest';

import BridgeSwitcher from './BridgeSwitcher';

afterEach(() => {
  cleanup();
});

describe('BridgeSwitcher', () => {
  const bridges = [
    { id: 'bridge-a', name: 'Bridge Alpha' },
    { id: 'bridge-b', name: 'Bridge Beta' },
    { id: 'bridge-c', name: 'Bridge Gamma' },
  ];

  test('bridge switcher: renders each bridge name and marks the active selection', () => {
    render(() => (
      <BridgeSwitcher bridges={bridges} activeBridgeId="bridge-b" onSelect={() => undefined} />
    ));

    const buttons = bridges.map(({ name }) => screen.getByRole('button', { name }));
    expect(buttons).toHaveLength(3);

    expect(screen.getByRole('button', { name: 'Bridge Beta' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Bridge Alpha' })).toHaveAttribute('aria-pressed', 'false');
  });

  test('bridge switcher: clicking a non-active bridge triggers selection callback', () => {
    const handleSelect = vi.fn();

    render(() => (
      <BridgeSwitcher bridges={bridges} activeBridgeId="bridge-a" onSelect={handleSelect} />
    ));

    fireEvent.click(screen.getByRole('button', { name: 'Bridge Gamma' }));

    expect(handleSelect).toHaveBeenCalledWith('bridge-c');
  });
});
