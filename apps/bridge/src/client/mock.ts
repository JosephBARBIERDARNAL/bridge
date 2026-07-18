import type {
  BridgeClient,
  ChatDetail,
  Message,
  RequestHandle,
  StreamListener,
  ToolCallRecord,
} from "../types";

const now = () => new Date().toISOString();
const id = () =>
  globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
const starterId = id();

const chats = new Map<string, ChatDetail>([
  [
    starterId,
    {
      chat: {
        id: starterId,
        title: "Welcome to Bridge",
        created_at: now(),
        updated_at: now(),
      },
      messages: [
        {
          id: id(),
          chat_id: starterId,
          role: "assistant",
          status: "complete",
          created_at: now(),
          thinking: "",
          tool_calls: "",
          content:
            "Your private local assistant is ready.\n\nThis browser preview is using mock data.",
        },
      ],
    },
  ],
]);

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export class MockBridgeClient implements BridgeClient {
  async health() {
    return {
      gateway: "ok",
      database: "ok",
      ollama: "mock",
      model: "gemma4:26b",
      model_available: true,
    };
  }

  async listChats() {
    return [...chats.values()]
      .map((value) => value.chat)
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at));
  }

  async createChat() {
    const chat = {
      id: id(),
      title: "New chat",
      created_at: now(),
      updated_at: now(),
    };
    chats.set(chat.id, { chat, messages: [] });
    return chat;
  }

  async getChat(chatId: string) {
    const chat = chats.get(chatId);
    if (!chat) throw new Error("Chat not found");
    return structuredClone(chat);
  }

  async renameChat(chatId: string, title: string) {
    const detail = chats.get(chatId);
    if (!detail) throw new Error("Chat not found");
    detail.chat = { ...detail.chat, title, updated_at: now() };
    return detail.chat;
  }

  async deleteChat(chatId: string) {
    chats.delete(chatId);
  }

  sendMessage(
    chatId: string,
    content: string,
    webSearch: boolean,
    listener: StreamListener,
  ) {
    const detail = chats.get(chatId);
    if (!detail) throw new Error("Chat not found");
    const user: Message = {
      id: id(),
      chat_id: chatId,
      role: "user",
      content,
      thinking: "",
      tool_calls: "",
      status: "complete",
      created_at: now(),
    };
    detail.messages.push(user);
    if (detail.chat.title === "New chat")
      detail.chat.title = content.trim().slice(0, 60);
    return this.stream(detail, user, webSearch, listener);
  }

  retryMessage(
    chatId: string,
    userMessageId: string,
    webSearch: boolean,
    listener: StreamListener,
  ) {
    const detail = chats.get(chatId);
    const user = detail?.messages.find(
      (message) => message.id === userMessageId && message.role === "user",
    );
    if (!detail || !user) throw new Error("Message not found");
    return this.stream(detail, user, webSearch, listener);
  }

  private stream(
    detail: ChatDetail,
    user: Message,
    webSearch: boolean,
    listener: StreamListener,
  ): RequestHandle {
    let cancelled = false;
    const assistant: Message = {
      id: id(),
      chat_id: detail.chat.id,
      role: "assistant",
      content: "",
      thinking: "",
      tool_calls: "",
      status: "streaming",
      created_at: now(),
    };
    detail.messages.push(assistant);
    listener.onStarted(user.id, assistant.id);
    const reasoning = "Preparing a concise local preview response.";
    const response = webSearch
      ? `Based on the mock search results, here is a preview answer to “${user.content}”. In real mode, the model on your Mac searches the web before answering.`
      : `This is a local preview response to “${user.content}”. In real mode, these tokens stream securely from the model running on your Mac.`;
    void (async () => {
      if (webSearch) {
        const query = user.content.trim().slice(0, 40);
        const record: ToolCallRecord = {
          name: "web_search",
          arguments: { query },
          status: "ok",
          result: {
            results: [
              {
                title: "Mock result one",
                url: "https://example.com/one",
                snippet: "A preview snippet for the first mock result.",
              },
              {
                title: "Mock result two",
                url: "https://example.com/two",
                snippet: "A preview snippet for the second mock result.",
              },
            ],
          },
          sources: [
            { title: "Mock result one", url: "https://example.com/one" },
            { title: "Mock result two", url: "https://example.com/two" },
          ],
        };
        listener.onToolCall(
          assistant.id,
          0,
          "web_search",
          JSON.stringify(record.arguments),
        );
        await delay(900);
        if (cancelled) return;
        assistant.tool_calls = JSON.stringify([record]);
        listener.onToolResult(
          assistant.id,
          0,
          "web_search",
          JSON.stringify(record),
        );
      }
      for (const token of reasoning.split(/(?<=\s)/)) {
        await delay(35);
        if (cancelled) return;
        assistant.thinking += token;
        listener.onThinkingDelta(assistant.id, token);
      }
      for (const token of response.split(/(?<=\s)/)) {
        await delay(35);
        if (cancelled) return;
        assistant.content += token;
        listener.onDelta(assistant.id, token);
      }
      assistant.status = "complete";
      detail.chat.updated_at = now();
      listener.onCompleted({ ...assistant });
    })();
    return {
      cancel: () => {
        cancelled = true;
      },
    };
  }
}
