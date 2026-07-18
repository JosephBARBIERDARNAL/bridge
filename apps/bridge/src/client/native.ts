import { NativeEventEmitter, NativeModules } from "react-native";
import type {
  BridgeClient,
  ChatDetail,
  ChatSummary,
  HealthStatus,
  RequestHandle,
  StreamListener,
} from "../types";

const module = NativeModules.BridgeCore;
const emitter = new NativeEventEmitter(module);

export class NativeBridgeClient implements BridgeClient {
  configure(baseUrl: string, token: string): Promise<void> {
    return module.configure(baseUrl, token);
  }
  health(): Promise<HealthStatus> {
    return module.health();
  }
  listChats(): Promise<ChatSummary[]> {
    return module.listChats();
  }
  createChat(): Promise<ChatSummary> {
    return module.createChat();
  }
  getChat(id: string): Promise<ChatDetail> {
    return module.getChat(id);
  }
  renameChat(id: string, title: string): Promise<ChatSummary> {
    return module.renameChat(id, title);
  }
  deleteChat(id: string): Promise<void> {
    return module.deleteChat(id);
  }

  sendMessage(
    chatId: string,
    content: string,
    webSearch: boolean,
    listener: StreamListener,
  ) {
    return this.subscribe(
      module.sendMessage(chatId, content, webSearch),
      listener,
    );
  }

  retryMessage(
    chatId: string,
    messageId: string,
    webSearch: boolean,
    listener: StreamListener,
  ) {
    return this.subscribe(
      module.retryMessage(chatId, messageId, webSearch),
      listener,
    );
  }

  private subscribe(
    requestIdPromise: Promise<string>,
    listener: StreamListener,
  ): RequestHandle {
    let requestId: string | undefined;
    const subscription = emitter.addListener("BridgeStreamEvent", (event) => {
      if (event.requestId !== requestId) return;
      if (event.type === "started")
        listener.onStarted(event.userMessageId, event.assistantMessageId);
      if (event.type === "thinking_delta")
        listener.onThinkingDelta(event.assistantMessageId, event.text);
      if (event.type === "delta")
        listener.onDelta(event.assistantMessageId, event.text);
      if (event.type === "tool_call")
        listener.onToolCall(
          event.assistantMessageId,
          event.callIndex,
          event.name,
          event.argumentsJson,
        );
      if (event.type === "tool_result")
        listener.onToolResult(
          event.assistantMessageId,
          event.callIndex,
          event.name,
          event.recordJson,
        );
      if (event.type === "completed") listener.onCompleted(event.message);
      if (event.type === "error") listener.onError(event.error);
    });
    void requestIdPromise.then((value) => {
      requestId = value;
    });
    return {
      cancel: () => {
        subscription.remove();
        if (requestId) void module.cancel(requestId);
      },
    };
  }
}
