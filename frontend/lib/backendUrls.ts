const PROD_API_BASE_URL = "https://battlecp.duckdns.org";

function trimTrailingSlash(url: string): string {
  return url.endsWith("/") ? url.slice(0, -1) : url;
}

export function getApiBaseUrl(): string {
  const configured = process.env.NEXT_PUBLIC_API_URL;
  if (configured) {
    return trimTrailingSlash(configured);
  }
  return PROD_API_BASE_URL;
}

export function getWsBaseUrl(): string {
  const configured = process.env.NEXT_PUBLIC_WS_URL;
  if (configured) {
    return trimTrailingSlash(configured);
  }
  return PROD_API_BASE_URL.replace("https://", "wss://");
}
