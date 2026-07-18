import { describe, expect, test } from "bun:test";
import type { Message, StreamListener } from "../types";
import { subscribeNativeStream } from "./nativeStream";

class FakeEmitter {
  private listeners = new Set<(event: Record<string, unknown>) => void>();
  removed = 0;

  addListener(
    _eventName: string,
    listener: (event: Record<string, unknown>) => void,
  ) {
    this.listeners.add(listener);
    return {
      remove: () => {
        if (this.listeners.delete(listener)) this.removed += 1;
      },
    };
  }

  emit(event: Record<string, unknown>) {
    for (const listener of [...this.listeners]) listener(event);
  }
}

const message: Message = {
  id: "assistant-1",
  chat_id: "chat-1",
  role: "assistant",
  content: "done",
  thinking: "",
  tool_calls: "",
  status: "complete",
  created_at: "2026-07-18T00:00:00Z",
};

function listener(overrides: Partial<StreamListener> = {}): StreamListener {
  return {
    onStarted() {},
    onThinkingDelta() {},
    onDelta() {},
    onToolCall() {},
    onToolResult() {},
    onCompleted() {},
    onError() {},
    ...overrides,
  };
}

describe("native stream subscription", () => {
  test("subscribes before starting and removes itself on completion", () => {
    const emitter = new FakeEmitter();
    const events: string[] = [];
    subscribeNativeStream(
      emitter,
      async (requestId) => {
        emitter.emit({
          requestId,
          type: "started",
          userMessageId: "user-1",
          assistantMessageId: "assistant-1",
        });
        emitter.emit({ requestId, type: "completed", message });
      },
      async () => {},
      listener({
        onStarted: () => events.push("started"),
        onCompleted: () => events.push("completed"),
      }),
    );

    expect(events).toEqual(["started", "completed"]);
    expect(emitter.removed).toBe(1);
  });

  test("cancels by the known request ID and removes the listener", () => {
    const emitter = new FakeEmitter();
    let startedId = "";
    let cancelledId = "";
    const handle = subscribeNativeStream(
      emitter,
      async (requestId) => {
        startedId = requestId;
      },
      async (requestId) => {
        cancelledId = requestId;
      },
      listener(),
    );
    handle.cancel();

    expect(cancelledId).toBe(startedId);
    expect(emitter.removed).toBe(1);
  });

  test("reports startup rejection and cleans up", async () => {
    const emitter = new FakeEmitter();
    const failure = new Promise<string>((resolve) =>
      subscribeNativeStream(
        emitter,
        async () => {
          throw new Error("start failed");
        },
        async () => {},
        listener({ onError: (error) => resolve(error.message) }),
      ),
    );
    expect(await failure).toBe("start failed");
    expect(emitter.removed).toBe(1);
  });

  test("does not report a late startup rejection after cancellation", async () => {
    const emitter = new FakeEmitter();
    let rejectStart: ((error: Error) => void) | undefined;
    let errors = 0;
    const handle = subscribeNativeStream(
      emitter,
      () =>
        new Promise((_resolve, reject) => {
          rejectStart = reject;
        }),
      async () => {},
      listener({ onError: () => (errors += 1) }),
    );
    handle.cancel();
    rejectStart?.(new Error("cancelled startup"));
    await Promise.resolve();
    expect(errors).toBe(0);
  });
});
