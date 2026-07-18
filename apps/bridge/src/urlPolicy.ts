export function normalizeHttpsUrl(raw: string) {
  try {
    const url = new URL(raw);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      !url.hostname
    )
      return undefined;
    url.hash = "";
    return url.toString();
  } catch {
    return undefined;
  }
}
