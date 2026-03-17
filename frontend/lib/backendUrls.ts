const PROD_API_BASE_URL = "https://battlecp-backend.whitepond-775af88d.centralindia.azurecontainerapps.io";

function trimTrailingSlash(url: string): string {
  return url.endsWith("/") ? url.slice(0, -1) : url;
}

function isLocalHost(hostname: string): boolean {
  return hostname === "localhost" || hostname === "127.0.0.1";
}

export function getApiBaseUrl(): string {
  const configured = process.env.NEXT_PUBLIC_API_URL;
  if (configured) {
    return trimTrailingSlash(configured);
  }

  if (typeof window !== "undefined" && isLocalHost(window.location.hostname)) {
    return "http://localhost:3000";
  }

  return PROD_API_BASE_URL;
}

export function getWsBaseUrl(): string {
  const configured = process.env.NEXT_PUBLIC_WS_URL;
  if (configured) {
    return trimTrailingSlash(configured);
  }

  if (typeof window !== "undefined" && isLocalHost(window.location.hostname)) {
    return "ws://localhost:3000";
  }

  return PROD_API_BASE_URL.replace("https://", "wss://");
}
