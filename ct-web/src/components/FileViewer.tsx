import { Show } from 'solid-js';

export interface FileOperation {
  type: 'read' | 'write';
  path: string;
  content?: string;
  status: 'pending' | 'success' | 'error';
  error?: string;
}

export interface FileViewerProps {
  operation: FileOperation;
}

export function FileViewer(props: FileViewerProps) {
  const getOperationIcon = () => {
    switch (props.operation.type) {
      case 'read':
        return '📖';
      case 'write':
        return '✏️';
      default:
        return '📄';
    }
  };

  const getStatusIcon = () => {
    switch (props.operation.status) {
      case 'pending':
        return '⏳';
      case 'success':
        return '✅';
      case 'error':
        return '❌';
      default:
        return '';
    }
  };

  const getStatusColor = () => {
    switch (props.operation.status) {
      case 'pending':
        return 'text-yellow-600';
      case 'success':
        return 'text-green-600';
      case 'error':
        return 'text-red-600';
      default:
        return 'text-gray-600';
    }
  };

  return (
    <div class="bg-blue-50 border border-blue-200 rounded-lg p-3 my-2">
      <div class="flex items-center space-x-2 mb-2">
        <span class="text-lg">{getOperationIcon()}</span>
        <span class="font-medium text-blue-900">
          {props.operation.type === 'read' ? 'Reading file' : 'Writing file'}
        </span>
        <span class={`text-sm ${getStatusColor()}`}>
          {getStatusIcon()} {props.operation.status}
        </span>
      </div>

      <div class="text-sm text-blue-800 mb-2">
        <strong>Path:</strong> {props.operation.path}
      </div>

      <Show when={props.operation.error}>
        <div class="text-sm text-red-600 mb-2">
          <strong>Error:</strong> {props.operation.error}
        </div>
      </Show>

      <Show when={props.operation.content && props.operation.type === 'read'}>
        <div class="mt-2">
          <div class="text-sm font-medium text-blue-900 mb-1">Content:</div>
          <div class="bg-white border border-blue-200 rounded p-2 text-xs font-mono text-gray-800 max-h-32 overflow-y-auto">
            <pre class="whitespace-pre-wrap">{props.operation.content}</pre>
          </div>
        </div>
      </Show>

      <Show when={props.operation.content && props.operation.type === 'write'}>
        <div class="mt-2">
          <div class="text-sm font-medium text-blue-900 mb-1">Content written:</div>
          <div class="bg-white border border-blue-200 rounded p-2 text-xs font-mono text-gray-800 max-h-32 overflow-y-auto">
            <pre class="whitespace-pre-wrap">{props.operation.content}</pre>
          </div>
        </div>
      </Show>
    </div>
  );
}

export function parseFileOperationFromToolCall(toolCall: any): FileOperation | null {
  // This would be called when processing session/update notifications
  // to extract file operation information from tool calls

  if (!toolCall || !toolCall.fields) return null;

  const { title, kind, status } = toolCall.fields;

  // Parse file path from title (e.g., "Write file: /path/to/file.txt")
  let path = '';
  let type: 'read' | 'write' = 'read';

  if (title) {
    if (title.startsWith('Write file:')) {
      type = 'write';
      path = title.replace('Write file:', '').trim();
    } else if (title.startsWith('Read file:')) {
      type = 'read';
      path = title.replace('Read file:', '').trim();
    }
  }

  if (!path) return null;

  let operationStatus: 'pending' | 'success' | 'error' = 'pending';
  if (status === 'completed') operationStatus = 'success';
  else if (status === 'failed') operationStatus = 'error';

  return {
    type,
    path,
    status: operationStatus
  };
}