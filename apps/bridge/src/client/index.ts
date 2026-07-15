import type { BridgeClient } from "../types";
import { NativeBridgeClient } from "./native";

export function createClient(): BridgeClient {
  return new NativeBridgeClient();
}
