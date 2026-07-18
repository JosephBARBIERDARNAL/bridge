import { Linking } from "react-native";
import { normalizeHttpsUrl } from "./urlPolicy";

export async function openHttpsUrl(raw: string) {
  const url = normalizeHttpsUrl(raw);
  if (!url) return false;
  await Linking.openURL(url);
  return true;
}
