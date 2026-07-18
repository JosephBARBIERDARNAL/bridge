import type { RequestHandle, StreamListener } from "../types";

type StreamEvent = Record<string, unknown> & {
  requestId?: string;
  type?: string;
};

type EventEmitter = {
  addListener(
    eventName: string,
    listener: (event: StreamEvent) => void,
  ): { remove(): void };
};

let requestSequence = 0;

const nextRequestId = () => `bridge-${Date.now()}-${requestSequence++}`;

export function subscribeNativeStream(
  emitter: EventEmitter,
  start: (requestId: string) => Promise<void>,
  cancel: (requestId: string) => Promise<void>,
  listener: StreamListener,
): RequestHandle {
  const requestId = nextRequestId();
  let closed = false;
  const subscription = emitter.addListener("BridgeStreamEvent", (event) => {
    if (event.requestId !== requestId) return;
    if (event.type === "started")
      listener.onStarted(
        event.userMessageId as string,
        event.assistantMessageId as string,
      );
    if (event.type === "thinking_delta")
      listener.onThinkingDelta(
        event.assistantMessageId as string,
        event.text as string,
      );
    if (event.type === "delta")
      listener.onDelta(
        event.assistantMessageId as string,
        event.text as string,
      );
    if (event.type === "tool_call")
      listener.onToolCall(
        event.assistantMessageId as string,
        event.callIndex as number,
        event.name as string,
        event.argumentsJson as string,
      );
    if (event.type === "tool_result")
      listener.onToolResult(
        event.assistantMessageId as string,
        event.callIndex as number,
        event.name as string,
        event.recordJson as string,
      );
    if (event.type === "completed") {
      try {
        listener.onCompleted(
          event.message as Parameters<StreamListener["onCompleted"]>[0],
        );
      } finally {
        cleanup();
      }
    }
    if (event.type === "error") {
      try {
        listener.onError(
          event.error as Parameters<StreamListener["onError"]>[0],
        );
      } finally {
        cleanup();
      }
    }
  });
  const cleanup = () => {
    if (closed) return;
    closed = true;
    subscription.remove();
  };
  void start(requestId).catch((error) => {
    if (closed) return;
    cleanup();
    listener.onError({
      code: "request_failed",
      message: error instanceof Error ? error.message : String(error),
      retryable: true,
    });
  });
  return {
    cancel: () => {
      cleanup();
      void cancel(requestId);
    },
  };
}
