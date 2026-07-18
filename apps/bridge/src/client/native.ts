import { NativeEventEmitter, NativeModules } from "react-native";
import type {
  BridgeClient,
  ChatDetail,
  ChatSummary,
  HealthStatus,
  RequestHandle,
  StreamListener,
} from "../types";
import { subscribeNativeStream } from "./nativeStream";

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
      (requestId) => module.sendMessage(requestId, chatId, content, webSearch),
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
      (requestId) =>
        module.retryMessage(requestId, chatId, messageId, webSearch),
      listener,
    );
  }

  private subscribe(
    start: (requestId: string) => Promise<void>,
    listener: StreamListener,
  ): RequestHandle {
    return subscribeNativeStream(
      emitter,
      start,
      (requestId) => module.cancel(requestId),
      listener,
    );
  }
}
