import type {
  BridgeClient,
  ChatDetail,
  ChatSummary,
  HealthStatus,
  Message,
  RequestHandle,
  StreamListener,
} from "../types";

export class WebGatewayClient implements BridgeClient {
  private readonly base = "/api/v1";

  health() {
    return this.json<HealthStatus>("GET", "/health");
  }
  listChats() {
    return this.json<ChatSummary[]>("GET", "/chats");
  }
  createChat() {
    return this.json<ChatSummary>("POST", "/chats");
  }
  getChat(id: string) {
    return this.json<ChatDetail>("GET", `/chats/${id}`);
  }
  renameChat(id: string, title: string) {
    return this.json<ChatSummary>("PATCH", `/chats/${id}`, { title });
  }
  async deleteChat(id: string) {
    await this.request("DELETE", `/chats/${id}`);
  }

  sendMessage(
    chatId: string,
    content: string,
    webSearch: boolean,
    listener: StreamListener,
  ) {
    return this.stream(
      `/chats/${chatId}/messages`,
      { content, web_search: webSearch },
      listener,
    );
  }

  retryMessage(
    chatId: string,
    userMessageId: string,
    webSearch: boolean,
    listener: StreamListener,
  ) {
    return this.stream(
      `/chats/${chatId}/messages/${userMessageId}/retry`,
      { web_search: webSearch },
      listener,
    );
  }

  private stream(
    path: string,
    body: unknown,
    listener: StreamListener,
  ): RequestHandle {
    const controller = new AbortController();
    void (async () => {
      try {
        const response = await fetch(`${this.base}${path}`, {
          method: "POST",
          signal: controller.signal,
          headers: body ? { "content-type": "application/json" } : {},
          body: body ? JSON.stringify(body) : undefined,
        });
        if (!response.ok || !response.body)
          throw new Error(await this.errorMessage(response));
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";
        let terminal = false;
        while (!terminal) {
          const { value, done } = await reader.read();
          if (done) {
            buffer += decoder.decode();
          } else {
            buffer += decoder.decode(value, { stream: true });
          }
          buffer = buffer.replace(/\r\n/g, "\n");
          let boundary: number;
          while ((boundary = buffer.indexOf("\n\n")) >= 0) {
            terminal = this.dispatch(buffer.slice(0, boundary), listener);
            buffer = buffer.slice(boundary + 2);
            if (terminal) break;
          }
          if (done) {
            if (!terminal && buffer.trim())
              terminal = this.dispatch(buffer, listener);
            break;
          }
        }
        if (!terminal)
          throw new Error("The response stream ended unexpectedly");
      } catch (error) {
        if (!controller.signal.aborted)
          listener.onError({
            code: "request_failed",
            message: String(error),
            retryable: true,
          });
      }
    })();
    return { cancel: () => controller.abort() };
  }

  private dispatch(frame: string, listener: StreamListener) {
    const event = frame.match(/^event:\s*(.+)$/m)?.[1];
    const data = frame.match(/^data:\s*(.+)$/m)?.[1];
    if (!event || !data) return false;
    const value = JSON.parse(data);
    if (event === "message_started")
      listener.onStarted(value.user_message_id, value.assistant_message_id);
    if (event === "thinking_delta")
      listener.onThinkingDelta(value.message_id, value.text);
    if (event === "delta") listener.onDelta(value.message_id, value.text);
    if (event === "tool_call")
      listener.onToolCall(
        value.message_id,
        value.call_index,
        value.name,
        value.arguments,
      );
    if (event === "tool_result")
      listener.onToolResult(
        value.message_id,
        value.call_index,
        value.name,
        value.record,
      );
    if (event === "message_completed") {
      listener.onCompleted(value as Message);
      return true;
    }
    if (event === "error") {
      listener.onError(value);
      return true;
    }
    return false;
  }

  private async request(method: string, path: string, body?: unknown) {
    const response = await fetch(`${this.base}${path}`, {
      method,
      headers: body ? { "content-type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!response.ok) throw new Error(await this.errorMessage(response));
    return response;
  }

  private async json<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    return (await this.request(method, path, body)).json() as Promise<T>;
  }

  private async errorMessage(response: Response) {
    try {
      return (
        (await response.json()).message ?? `Request failed (${response.status})`
      );
    } catch {
      return `Request failed (${response.status})`;
    }
  }
}
