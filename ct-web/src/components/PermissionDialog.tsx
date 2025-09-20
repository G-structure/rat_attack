import { Show } from 'solid-js';

export interface PermissionRequest {
  id: string;
  sessionId: string;
  toolCall: {
    tool: string;
    input: any;
  };
  options: PermissionOption[];
}

export interface PermissionOption {
  id: string;
  label: string;
  kind: 'allow_once' | 'allow_always' | 'reject_once' | 'reject_always';
}

export interface PermissionDialogProps {
  request: PermissionRequest | null;
  onResponse: (optionId: string) => void;
  onCancel: () => void;
}

export function PermissionDialog(props: PermissionDialogProps) {
  const handleOptionClick = (optionId: string) => {
    props.onResponse(optionId);
  };

  const getOptionButtonClass = (kind: string) => {
    switch (kind) {
      case 'allow_once':
      case 'allow_always':
        return 'bg-green-600 hover:bg-green-700 text-white';
      case 'reject_once':
      case 'reject_always':
        return 'bg-red-600 hover:bg-red-700 text-white';
      default:
        return 'bg-gray-600 hover:bg-gray-700 text-white';
    }
  };

  return (
    <Show when={props.request}>
      <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
        <div class="bg-white rounded-lg shadow-xl max-w-md w-full mx-4">
          {/* Header */}
          <div class="px-6 py-4 border-b border-gray-200">
            <h3 class="text-lg font-semibold text-gray-900">
              Permission Required
            </h3>
          </div>

          {/* Content */}
          <div class="px-6 py-4">
            <div class="mb-4">
              <p class="text-sm text-gray-600 mb-2">
                The agent wants to perform the following action:
              </p>
              <div class="bg-gray-50 border border-gray-200 rounded-lg p-3">
                <div class="font-medium text-gray-900">
                  {props.request?.toolCall.tool}
                </div>
                <div class="text-sm text-gray-600 mt-1">
                  {JSON.stringify(props.request?.toolCall.input, null, 2)}
                </div>
              </div>
            </div>

            <p class="text-sm text-gray-600 mb-4">
              How would you like to respond?
            </p>

            {/* Options */}
            <div class="space-y-2">
              {props.request?.options.map((option) => (
                <button
                  key={option.id}
                  class={`w-full px-4 py-2 rounded-lg font-medium text-left transition-colors ${getOptionButtonClass(option.kind)}`}
                  onClick={() => handleOptionClick(option.id)}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>

          {/* Footer */}
          <div class="px-6 py-4 border-t border-gray-200 flex justify-end">
            <button
              class="px-4 py-2 text-gray-600 hover:text-gray-800 font-medium"
              onClick={props.onCancel}
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}