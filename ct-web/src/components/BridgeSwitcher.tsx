import type { Component } from 'solid-js';
import { For } from 'solid-js';

type Bridge = {
  id: string;
  name: string;
};

type BridgeSwitcherProps = {
  bridges: Bridge[];
  activeBridgeId?: string;
  onSelect: (bridgeId: string) => void;
};

const BridgeSwitcher: Component<BridgeSwitcherProps> = (props) => {
  const isActive = (bridgeId: string) => bridgeId === props.activeBridgeId;

  const handleSelect = (bridgeId: string) => {
    if (!isActive(bridgeId)) {
      props.onSelect(bridgeId);
    }
  };

  return (
    <nav
      aria-label="Connected bridges"
      class="flex gap-2 overflow-x-auto rounded-full bg-slate-100 p-2 text-sm"
    >
      <For each={props.bridges}>
        {(bridge) => {
          const active = isActive(bridge.id);

          return (
            <button
              type="button"
              class={`whitespace-nowrap rounded-full px-4 py-2 font-medium transition focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-500 ${
                active
                  ? 'bg-slate-900 text-white shadow-sm'
                  : 'bg-white text-slate-700 hover:bg-slate-200'
              }`}
              aria-pressed={active ? 'true' : 'false'}
              onClick={() => handleSelect(bridge.id)}
            >
              {bridge.name}
            </button>
          );
        }}
      </For>
    </nav>
  );
};

export default BridgeSwitcher;
