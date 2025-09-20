import { createSignal, createEffect, Show, For, onMount } from 'solid-js';
import { createWebSocketClient } from './WebSocketClient';

interface Message {
  id: string;
  type: 'user' | 'agent' | 'system';
  content: string;
  timestamp: Date;
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

  const wsClient = createWebSocketClient();

  // Auto-connect once on mount
  onMount(() => {
    wsClient.connect('ws://127.0.0.1:8137');
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

  const addMessage = (type: 'user' | 'agent' | 'system', content: string) => {
    const message: Message = {
      id: crypto.randomUUID(),
      type,
      content,
      timestamp: new Date()
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
      // For dummy agent, just add a simple response since it doesn't stream
      addMessage('agent', `Echo: ${text}`);
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
                class={`max-w-xs md:max-w-md lg:max-w-lg px-4 py-2 rounded-lg ${
                  message.type === 'user'
                    ? 'bg-blue-600 text-white'
                    : message.type === 'agent'
                    ? 'bg-white border border-gray-200 text-gray-900'
                    : 'bg-gray-100 text-gray-600 text-sm'
                }`}
              >
                <div class="whitespace-pre-wrap">{message.content}</div>
                <div
                  class={`text-xs mt-1 ${
                    message.type === 'user' ? 'text-blue-100' : 'text-gray-400'
                  }`}
                >
                  {message.timestamp.toLocaleTimeString()}
                </div>
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
    </div>
  );
}