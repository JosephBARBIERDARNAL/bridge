import type {
  BridgeClient,
  ChatDetail,
  Message,
  RequestHandle,
  StreamListener,
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

  sendMessage(chatId: string, content: string, listener: StreamListener) {
    const detail = chats.get(chatId);
    if (!detail) throw new Error("Chat not found");
    const user: Message = {
      id: id(),
      chat_id: chatId,
      role: "user",
      content,
      status: "complete",
      created_at: now(),
    };
    detail.messages.push(user);
    if (detail.chat.title === "New chat")
      detail.chat.title = content.trim().slice(0, 60);
    return this.stream(detail, user, listener);
  }

  retryMessage(
    chatId: string,
    userMessageId: string,
    listener: StreamListener,
  ) {
    const detail = chats.get(chatId);
    const user = detail?.messages.find(
      (message) => message.id === userMessageId && message.role === "user",
    );
    if (!detail || !user) throw new Error("Message not found");
    return this.stream(detail, user, listener);
  }

  private stream(
    detail: ChatDetail,
    user: Message,
    listener: StreamListener,
  ): RequestHandle {
    let cancelled = false;
    const assistant: Message = {
      id: id(),
      chat_id: detail.chat.id,
      role: "assistant",
      content: "",
      status: "streaming",
      created_at: now(),
    };
    detail.messages.push(assistant);
    listener.onStarted(user.id, assistant.id);
    const response = `This is a local preview response to “${user.content}”. In real mode, these tokens stream securely from the model running on your Mac.`;
    void (async () => {
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
