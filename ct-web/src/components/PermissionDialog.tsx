import { Show } from 'solid-js';
import type { PermissionRequest as WSPermissionRequest, PermissionOption } from './WebSocketClient';

export interface PermissionDialogRequest {
  requestId: string | number;
  request: WSPermissionRequest;
}

export interface PermissionDialogProps {
  request: PermissionDialogRequest | null;
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
                  {props.request?.request.params.toolCall.fields.title || 'File operation'}
                </div>
                <div class="text-sm text-gray-600 mt-1">
                  Type: {props.request?.request.params.toolCall.fields.kind || 'unknown'}
                </div>
              </div>
            </div>

            <p class="text-sm text-gray-600 mb-4">
              How would you like to respond?
            </p>

            {/* Options */}
            <div class="space-y-2">
              {props.request?.request.params.options.map((option) => (
                <button
                  key={option.id}
                  class={`w-full px-4 py-2 rounded-lg font-medium text-left transition-colors ${getOptionButtonClass(option.kind)}`}
                  onClick={() => handleOptionClick(option.id)}
                >
                  {option.name}
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