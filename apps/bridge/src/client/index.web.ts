import type { BridgeClient } from "../types";
import { MockBridgeClient } from "./mock";
import { WebGatewayClient } from "./web";

export function createClient(): BridgeClient {
  return import.meta.env.VITE_PREVIEW_MODE === "real"
    ? new WebGatewayClient()
    : new MockBridgeClient();
}
