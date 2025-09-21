import { createSignal, createEffect, onCleanup } from 'solid-js';

// ACP Types
export interface InitializeRequest {
  protocolVersion: number;
  clientCapabilities: {
    fs: {
      readTextFile: boolean;
      writeTextFile: boolean;
    };
    terminal: boolean;
  };
}

export interface InitializeResponse {
  protocolVersion: number;
  agentCapabilities: {
    loadSession?: boolean;
    promptCapabilities?: {
      image?: boolean;
      embeddedContext?: boolean;
    };
    mcpCapabilities?: {
      http?: boolean;
      sse?: boolean;
    };
  };
  authMethods: any[];
  _meta?: {
    bridgeId?: string;
    projectRoot?: string;
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

export interface SessionUpdateNotification {
  method: 'session/update';
  params: {
    sessionId: string;
    content: {
      type: string;
      text?: string;
    };
  };
}

export interface PermissionOption {
  id: string;
  name: string;
  kind: 'allow_once' | 'allow_always' | 'reject_once' | 'reject_always';
}

export interface PermissionRequest {
  method: 'session/request_permission';
  params: {
    sessionId: string;
    toolCall: {
      id: string;
      fields: {
        kind?: string;
        title?: string;
        status?: string;
      };
    };
    options: PermissionOption[];
  };
}

export interface PermissionResponse {
  outcome: {
    outcome: 'selected' | 'cancelled';
    optionId?: string;
  };
}

export function createWebSocketClient() {
  const [ws, setWs] = createSignal<WebSocket | null>(null);
  const [connectionState, setConnectionState] = createSignal<ConnectionState>({
    status: 'disconnected'
  });

  let requestId = 1;
  let connectUrl: string | null = null;
  let projectRoot: string | null = null;
  let notificationHandler: ((notification: any) => void) | null = null;
  let permissionRequestHandler: ((request: PermissionRequest, requestId: string | number) => void) | null = null;
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

        if (message.id !== undefined && message.method !== undefined) {
          // This is a request from the server (e.g., permission request)
          console.log('Received request from server:', message);
          if (message.method === 'session/request_permission' && permissionRequestHandler) {
            permissionRequestHandler(message, message.id);
          }
        } else if (message.id !== undefined) {
          // This is a response to a request we sent
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
          if (notificationHandler) {
            notificationHandler(message);
          }
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
      protocolVersion: 1,
      clientCapabilities: {
        fs: {
          readTextFile: true,
          writeTextFile: true
        },
        terminal: true
      }
    };

    const response = await sendRequest('initialize', request);

    // Update connection state with bridge ID and store project root if available
    if (response._meta?.bridgeId) {
      setConnectionState(prev => ({
        ...prev,
        bridgeId: response._meta.bridgeId
      }));
    }

    // Store project root for use in session creation
    if (response._meta?.projectRoot) {
      projectRoot = response._meta.projectRoot;
    }

    return response;
  };

  const newSession = async (): Promise<NewSessionResponse> => {
    const request: NewSessionRequest = {
      cwd: projectRoot || '/tmp', // Use project root from bridge, fallback to /tmp
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

  const setNotificationHandler = (handler: ((notification: any) => void) | null) => {
    notificationHandler = handler;
  };

  const setPermissionRequestHandler = (handler: ((request: PermissionRequest, requestId: string | number) => void) | null) => {
    permissionRequestHandler = handler;
  };

  const sendPermissionResponse = async (requestId: string | number, response: PermissionResponse): Promise<void> => {
    const currentWs = ws();
    if (!currentWs || currentWs.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket not connected');
    }

    const responseMessage = {
      jsonrpc: '2.0',
      id: requestId,
      result: response
    };

    currentWs.send(JSON.stringify(responseMessage));
  };

  return {
    connectionState,
    connect,
    disconnect,
    initialize,
    newSession,
    prompt,
    sendRequest,
    setNotificationHandler,
    setPermissionRequestHandler,
    sendPermissionResponse
  };
}