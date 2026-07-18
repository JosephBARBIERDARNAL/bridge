export type ChatSummary = {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
};

export type Message = {
  id: string;
  chat_id: string;
  role: "user" | "assistant";
  content: string;
  thinking: string;
  tool_calls: string;
  status: "complete" | "streaming" | "failed";
  created_at: string;
};

export type ToolSource = { title: string; url: string };

export type ToolCallRecord = {
  name: string;
  arguments: Record<string, unknown>;
  status: "running" | "ok" | "error";
  result?: unknown;
  sources?: ToolSource[];
};

export function parseToolCalls(text: string | undefined): ToolCallRecord[] {
  if (!text) return [];
  try {
    const parsed = JSON.parse(text);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export type ChatDetail = { chat: ChatSummary; messages: Message[] };
export type HealthStatus = {
  gateway: string;
  database: string;
  ollama: string;
  model: string;
  model_available: boolean;
};

export type StreamFailure = {
  code: string;
  message: string;
  retryable: boolean;
};
export type StreamListener = {
  onStarted(userMessageId: string, assistantMessageId: string): void;
  onThinkingDelta(assistantMessageId: string, text: string): void;
  onDelta(assistantMessageId: string, text: string): void;
  onToolCall(
    assistantMessageId: string,
    callIndex: number,
    name: string,
    argumentsJson: string,
  ): void;
  onToolResult(
    assistantMessageId: string,
    callIndex: number,
    name: string,
    recordJson: string,
  ): void;
  onCompleted(message: Message): void;
  onError(error: StreamFailure): void;
};
export type RequestHandle = { cancel(): void };

export interface BridgeClient {
  configure?(baseUrl: string, token: string): Promise<void>;
  health(): Promise<HealthStatus>;
  listChats(): Promise<ChatSummary[]>;
  createChat(): Promise<ChatSummary>;
  getChat(chatId: string): Promise<ChatDetail>;
  renameChat(chatId: string, title: string): Promise<ChatSummary>;
  deleteChat(chatId: string): Promise<void>;
  sendMessage(
    chatId: string,
    content: string,
    webSearch: boolean,
    listener: StreamListener,
  ): RequestHandle;
  retryMessage(
    chatId: string,
    userMessageId: string,
    webSearch: boolean,
    listener: StreamListener,
  ): RequestHandle;
}
