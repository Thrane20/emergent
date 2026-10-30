import "server-only";
import { randomBytes, timingSafeEqual } from "node:crypto";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { settingsDirectory } from "./ai-settings";

/** Per-install bearer token shared with the stdio bridge through the IDE data directory; never stored in the project. */
export function mcpTokenPath() { return path.join(settingsDirectory(), "mcp-token"); }

export async function mcpToken(): Promise<string> {
  try { const token = (await readFile(mcpTokenPath(), "utf8")).trim(); if (token.length >= 32) return token; }
  catch (error) { if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error; }
  const token = randomBytes(32).toString("base64url");
  await mkdir(settingsDirectory(), { recursive: true, mode: 0o700 });
  try { await writeFile(mcpTokenPath(), `${token}\n`, { mode: 0o600, flag: "wx" }); }
  catch (error) { if (error instanceof Error && "code" in error && error.code === "EEXIST") return (await readFile(mcpTokenPath(), "utf8")).trim(); throw error; }
  await chmod(mcpTokenPath(), 0o600);
  return token;
}

const LOOPBACK = new Set(["127.0.0.1", "localhost", "[::1]", "::1"]);

/** Returns a rejection response, or null when the request may reach the MCP server. */
export async function mcpRejection(request: Request): Promise<Response | null> {
  const url = new URL(request.url);
  const host = (request.headers.get("host") ?? url.host).replace(/:\d+$/, "");
  // DNS-rebinding defence: only loopback hosts, and browsers may only call from the IDE's own origin.
  if (!LOOPBACK.has(host)) return deny(403, "The Emergent MCP endpoint only accepts loopback requests.");
  const origin = request.headers.get("origin");
  if (origin && origin !== url.origin) return deny(403, "Cross-origin MCP requests are not allowed.");
  const supplied = request.headers.get("authorization")?.match(/^Bearer\s+(\S+)$/i)?.[1] ?? "";
  const expected = await mcpToken();
  const a = Buffer.from(supplied), b = Buffer.from(expected);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return deny(401, "Missing or invalid MCP token. Connect through `pnpm mcp:stdio`, which reads it from the IDE data directory.");
  return null;
}

function deny(status: number, message: string) {
  return Response.json({ jsonrpc: "2.0", error: { code: -32001, message }, id: null }, { status, headers: { "Cache-Control": "no-store" } });
}
