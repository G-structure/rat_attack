import { createSignal, createEffect, Show, For, onMount } from 'solid-js';
import { createWebSocketClient, type PermissionRequest } from './WebSocketClient';
import { PermissionDialog, type PermissionDialogRequest } from './PermissionDialog';
import { FileViewer, parseFileOperationFromToolCall, type FileOperation } from './FileViewer';

interface Message {
  id: string;
  type: 'user' | 'agent' | 'system' | 'file_operation';
  content: string;
  timestamp: Date;
  fileOperation?: FileOperation;
}

interface Session {
  id: string;
  messages: Message[];
}

export function ChatInterface() {
  const [currentSession, setCurrentSession] = createSignal<Session | null>(null);
  const [inputText, setInputText] = createSignal('');
  const [isLoading, setIsLoading] = createSignal(false);
  const [messages, setMessages] = createSignal<Message[]>([]);
  const [isInitializing, setIsInitializing] = createSignal(false);
  const [permissionRequest, setPermissionRequest] = createSignal<PermissionDialogRequest | null>(null);

  const wsClient = createWebSocketClient();

  // Auto-connect once on mount
  onMount(() => {
    wsClient.connect('ws://127.0.0.1:8137');

    // Set up notification handler for streaming responses
    wsClient.setNotificationHandler((notification) => {
      console.log('Processing notification:', notification);
      if (notification.method === 'session/update') {
        const params = notification.params;
        console.log('Notification params:', params);

        // Handle different possible structures
        if (params && typeof params === 'object') {
          // Try different ways the notification might be structured
          let sessionId = params.sessionId || params.session_id;
          let textContent = null;

          // Check for tool call updates (file operations, etc.)
          if (params.toolCall || params.update?.toolCall) {
            const toolCall = params.toolCall || params.update?.toolCall;
            const fileOperation = parseFileOperationFromToolCall(toolCall);

            if (fileOperation && sessionId === currentSession()?.id) {
              addMessage('file_operation',
                `${fileOperation.type === 'read' ? 'Reading' : 'Writing'} file: ${fileOperation.path}`,
                fileOperation
              );
              return; // Don't process as regular text message
            }
          }

          // Look for text content in various possible structures
          if (params.update && params.update.content && params.update.content.text) {
            textContent = params.update.content.text;
          } else if (params.content) {
            if (typeof params.content === 'string') {
              textContent = params.content;
            } else if (params.content.text) {
              textContent = params.content.text;
            }
          } else if (params.text) {
            textContent = params.text;
          } else if (params.message) {
            textContent = params.message;
          }

          if (sessionId === currentSession()?.id && textContent) {
            addMessage('agent', textContent);
          } else {
            console.log('Notification received but not processed:', {
              sessionId,
              currentSessionId: currentSession()?.id,
              textContent,
              params
            });
          }
        }
      }
    });

    // Set up permission request handler
    wsClient.setPermissionRequestHandler((request: PermissionRequest, requestId: string | number) => {
      console.log('Received permission request:', request);
      setPermissionRequest({
        requestId,
        request
      });
    });
  });

  // Auto-initialize when connected (only when status changes to 'connected')
  createEffect(() => {
    const state = wsClient.connectionState();
    if (state.status === 'connected' && !currentSession() && !isInitializing()) {
      setIsInitializing(true);
      initializeConnection();
    }
  });

  const initializeConnection = async () => {
    try {
      addMessage('system', 'Initializing connection...');

      const initResponse = await wsClient.initialize();
      addMessage('system', `Connected to bridge: ${initResponse._meta?.bridgeId || 'unknown'}`);

      const sessionResponse = await wsClient.newSession();
      setCurrentSession({
        id: sessionResponse.sessionId,
        messages: []
      });

      addMessage('system', `Session created: ${sessionResponse.sessionId}`);
      addMessage('agent', 'Hello! I\'m ready to help you. What would you like to do?');
    } catch (error) {
      addMessage('system', `Error during initialization: ${error}`);
    } finally {
      setIsInitializing(false);
    }
  };

  const addMessage = (type: 'user' | 'agent' | 'system' | 'file_operation', content: string, fileOperation?: FileOperation) => {
    const message: Message = {
      id: crypto.randomUUID(),
      type,
      content,
      timestamp: new Date(),
      fileOperation
    };
    setMessages(prev => [...prev, message]);
  };

  const sendMessage = async () => {
    const text = inputText().trim();
    if (!text || !currentSession() || isLoading()) return;

    addMessage('user', text);
    setInputText('');
    setIsLoading(true);

    try {
      await wsClient.prompt(currentSession()!.id, text);
      // Response will be handled via session/update notifications
    } catch (error) {
      addMessage('system', `Error sending message: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const handlePermissionResponse = async (optionId: string) => {
    const currentRequest = permissionRequest();
    if (!currentRequest) return;

    try {
      await wsClient.sendPermissionResponse(currentRequest.requestId, {
        outcome: {
          outcome: 'selected',
          optionId
        }
      });
      addMessage('system', `Permission granted: ${optionId}`);
    } catch (error) {
      addMessage('system', `Error responding to permission request: ${error}`);
    } finally {
      setPermissionRequest(null);
    }
  };

  const handlePermissionCancel = async () => {
    const currentRequest = permissionRequest();
    if (!currentRequest) return;

    try {
      await wsClient.sendPermissionResponse(currentRequest.requestId, {
        outcome: {
          outcome: 'cancelled'
        }
      });
      addMessage('system', 'Permission request cancelled');
    } catch (error) {
      addMessage('system', `Error cancelling permission request: ${error}`);
    } finally {
      setPermissionRequest(null);
    }
  };

  const getConnectionStatusColor = () => {
    const state = wsClient.connectionState();
    switch (state.status) {
      case 'connected': return 'text-green-600';
      case 'connecting': return 'text-yellow-600';
      case 'error': return 'text-red-600';
      default: return 'text-gray-600';
    }
  };

  const getConnectionStatusText = () => {
    const state = wsClient.connectionState();
    switch (state.status) {
      case 'connected': return `Connected${state.bridgeId ? ` (${state.bridgeId})` : ''}`;
      case 'connecting': return 'Connecting...';
      case 'error': return `Error: ${state.error}`;
      default: return 'Disconnected';
    }
  };

  return (
    <div class="flex flex-col h-screen bg-gray-50">
      {/* Header */}
      <div class="bg-white border-b border-gray-200 p-4">
        <div class="flex justify-between items-center">
          <h1 class="text-xl font-semibold text-gray-900">CT-Bridge Chat</h1>
          <div class={`text-sm ${getConnectionStatusColor()}`}>
            {getConnectionStatusText()}
          </div>
        </div>
        <Show when={currentSession()}>
          <div class="text-sm text-gray-500 mt-1">
            Session: {currentSession()?.id}
          </div>
        </Show>
      </div>

      {/* Messages */}
      <div class="flex-1 overflow-y-auto p-4 space-y-4">
        <For each={messages()}>
          {(message) => (
            <div class={`flex ${message.type === 'user' ? 'justify-end' : 'justify-start'}`}>
              <div
                class={`max-w-xs md:max-w-md lg:max-w-lg ${
                  message.type === 'file_operation' ? 'w-full max-w-full' : 'px-4 py-2 rounded-lg'
                } ${
                  message.type === 'user'
                    ? 'bg-blue-600 text-white px-4 py-2 rounded-lg'
                    : message.type === 'agent'
                    ? 'bg-white border border-gray-200 text-gray-900 px-4 py-2 rounded-lg'
                    : message.type === 'file_operation'
                    ? ''
                    : 'bg-gray-100 text-gray-600 text-sm px-4 py-2 rounded-lg'
                }`}
              >
                <Show when={message.type === 'file_operation' && message.fileOperation}>
                  <FileViewer operation={message.fileOperation!} />
                </Show>

                <Show when={message.type !== 'file_operation'}>
                  <div class="whitespace-pre-wrap">{message.content}</div>
                  <div
                    class={`text-xs mt-1 ${
                      message.type === 'user' ? 'text-blue-100' : 'text-gray-400'
                    }`}
                  >
                    {message.timestamp.toLocaleTimeString()}
                  </div>
                </Show>
              </div>
            </div>
          )}
        </For>

        <Show when={isLoading()}>
          <div class="flex justify-start">
            <div class="bg-white border border-gray-200 text-gray-900 px-4 py-2 rounded-lg">
              <div class="flex items-center space-x-2">
                <div class="animate-spin h-4 w-4 border-2 border-gray-300 border-t-blue-600 rounded-full"></div>
                <span>Agent is thinking...</span>
              </div>
            </div>
          </div>
        </Show>
      </div>

      {/* Input */}
      <div class="bg-white border-t border-gray-200 p-4">
        <div class="flex space-x-2">
          <textarea
            class="flex-1 resize-none border border-gray-300 rounded-lg px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            placeholder="Type your message..."
            rows="2"
            value={inputText()}
            onInput={(e) => setInputText(e.currentTarget.value)}
            onKeyDown={handleKeyDown}
            disabled={!currentSession() || isLoading()}
          />
          <button
            class={`px-4 py-2 rounded-lg font-medium ${
              !currentSession() || isLoading() || !inputText().trim()
                ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                : 'bg-blue-600 text-white hover:bg-blue-700'
            }`}
            onClick={sendMessage}
            disabled={!currentSession() || isLoading() || !inputText().trim()}
          >
            Send
          </button>
        </div>
      </div>

      {/* Permission Dialog */}
      <PermissionDialog
        request={permissionRequest()}
        onResponse={handlePermissionResponse}
        onCancel={handlePermissionCancel}
      />
    </div>
  );
}