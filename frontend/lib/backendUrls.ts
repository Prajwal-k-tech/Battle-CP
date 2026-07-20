function isLocalHost(hostname: string): boolean {
  return hostname === "localhost" || hostname === "127.0.0.1";
}

export function getApiBaseUrl(): string {
  if (typeof window !== "undefined" && isLocalHost(window.location.hostname)) {
    return "http://localhost:3000";
  }
  return "";
}

export function getWsBaseUrl(): string {
  if (typeof window !== "undefined" && isLocalHost(window.location.hostname)) {
    return "ws://localhost:3000";
  }
  if (typeof window !== "undefined") {
    const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
    return `${proto}//${window.location.host}`;
  }
  return "";
}
