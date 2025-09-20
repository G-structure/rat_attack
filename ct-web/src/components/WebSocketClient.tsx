import { createSignal, createEffect, onCleanup } from 'solid-js';

// ACP Types
export interface InitializeRequest {
  protocolVersion: string;
  clientCapabilities: {
    fs: {
      readTextFile: boolean;
      writeTextFile: boolean;
    };
    terminal: boolean;
  };
}

export interface InitializeResponse {
  protocolVersion: string;
  agentCapabilities: {
    loadSession: boolean;
  };
  authMethods: string[];
  _meta?: {
    bridgeId?: string;
  };
}

export interface NewSessionRequest {
  cwd: string;
  mcpServers?: any[];
}

export interface NewSessionResponse {
  sessionId: string;
  modes?: any;
}

export interface PromptRequest {
  sessionId: string;
  prompt: {
    type: string;
    text: string;
  }[];
}

export interface PromptResponse {
  stopReason: string;
}

// JSON-RPC Message Types
interface JsonRpcRequest {
  jsonrpc: string;
  id: string | number;
  method: string;
  params: any;
}

interface JsonRpcResponse {
  jsonrpc: string;
  id: string | number;
  result?: any;
  error?: {
    code: number;
    message: string;
    data?: any;
  };
}

interface JsonRpcNotification {
  jsonrpc: string;
  method: string;
  params: any;
}

export interface ConnectionState {
  status: 'disconnected' | 'connecting' | 'connected' | 'error';
  bridgeId?: string;
  error?: string;
}

export interface WebSocketClientProps {
  url: string;
  onConnectionChange: (state: ConnectionState) => void;
  onMessage: (message: any) => void;
}

export function createWebSocketClient() {
  const [ws, setWs] = createSignal<WebSocket | null>(null);
  const [connectionState, setConnectionState] = createSignal<ConnectionState>({
    status: 'disconnected'
  });

  let requestId = 1;
  let connectUrl: string | null = null;
  const pendingRequests = new Map<string | number, {
    resolve: (value: any) => void;
    reject: (error: any) => void;
  }>();

  const connect = (url: string) => {
    // Prevent multiple simultaneous connection attempts
    const currentState = connectionState();
    if (currentState.status === 'connecting' || currentState.status === 'connected') {
      console.log('Connection already in progress or established, skipping');
      return;
    }

    // Check if we're already connected to this URL
    if (connectUrl === url && ws()?.readyState === WebSocket.OPEN) {
      console.log('Already connected to this URL, skipping');
      return;
    }

    if (ws()) {
      ws()?.close();
    }

    connectUrl = url;
    setConnectionState({ status: 'connecting' });

    const websocket = new WebSocket(url, ['acp.jsonrpc.v1']);

    websocket.onopen = () => {
      setWs(websocket);
      setConnectionState({ status: 'connected' });
    };

    websocket.onclose = () => {
      setConnectionState({ status: 'disconnected' });
      setWs(null);
      connectUrl = null;
      // Clear pending requests
      pendingRequests.forEach(({ reject }) => {
        reject(new Error('WebSocket connection closed'));
      });
      pendingRequests.clear();
    };

    websocket.onerror = (error) => {
      setConnectionState({
        status: 'error',
        error: 'WebSocket connection failed'
      });
    };

    websocket.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);

        if (message.id !== undefined) {
          // This is a response to a request
          const pending = pendingRequests.get(message.id);
          if (pending) {
            pendingRequests.delete(message.id);
            if (message.error) {
              pending.reject(new Error(message.error.message));
            } else {
              pending.resolve(message.result);
            }
          }
        } else {
          // This is a notification
          console.log('Received notification:', message);
        }
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error);
      }
    };
  };

  const sendRequest = async (method: string, params: any): Promise<any> => {
    const currentWs = ws();
    if (!currentWs) {
      throw new Error('WebSocket not initialized');
    }

    // Wait for WebSocket to be fully open if it's still connecting
    if (currentWs.readyState === WebSocket.CONNECTING) {
      await new Promise((resolve, reject) => {
        const checkReady = () => {
          if (currentWs.readyState === WebSocket.OPEN) {
            resolve(undefined);
          } else if (currentWs.readyState === WebSocket.CLOSED) {
            reject(new Error('WebSocket connection failed'));
          } else {
            setTimeout(checkReady, 10);
          }
        };
        checkReady();
      });
    }

    if (currentWs.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket not connected');
    }

    const id = requestId++;
    const request: JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method,
      params
    };

    return new Promise((resolve, reject) => {
      pendingRequests.set(id, { resolve, reject });

      // Set a timeout for the request
      setTimeout(() => {
        if (pendingRequests.has(id)) {
          pendingRequests.delete(id);
          reject(new Error('Request timeout'));
        }
      }, 30000); // 30 second timeout

      currentWs.send(JSON.stringify(request));
    });
  };

  const initialize = async (): Promise<InitializeResponse> => {
    const request: InitializeRequest = {
      protocolVersion: '1.0.0',
      clientCapabilities: {
        fs: {
          readTextFile: true,
          writeTextFile: true
        },
        terminal: true
      }
    };

    const response = await sendRequest('initialize', request);

    // Update connection state with bridge ID if available
    if (response._meta?.bridgeId) {
      setConnectionState(prev => ({
        ...prev,
        bridgeId: response._meta.bridgeId
      }));
    }

    return response;
  };

  const newSession = async (): Promise<NewSessionResponse> => {
    const request: NewSessionRequest = {
      cwd: '/tmp', // Default working directory for demo
      mcpServers: []
    };
    return await sendRequest('session/new', request);
  };

  const prompt = async (sessionId: string, text: string): Promise<PromptResponse> => {
    // Bridge expects simple format: { sessionId, prompt }
    const request = {
      sessionId,
      prompt: text
    };

    return await sendRequest('session/prompt', request);
  };

  const disconnect = () => {
    const currentWs = ws();
    if (currentWs) {
      currentWs.close();
      setWs(null);
    }
    setConnectionState({ status: 'disconnected' });
  };

  return {
    connectionState,
    connect,
    disconnect,
    initialize,
    newSession,
    prompt,
    sendRequest
  };
}