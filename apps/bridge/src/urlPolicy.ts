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
    return url.toString().split("#", 1)[0];
  } catch {
    return undefined;
  }
}
