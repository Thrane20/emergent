#!/usr/bin/env node
// Stdio bridge for MCP clients (Claude Code, Codex). Forwards newline-delimited JSON-RPC to the running IDE's
// /api/mcp endpoint, so every request shares the IDE's validation, locks and inspection state.
// stdout carries protocol messages only; diagnostics go to stderr.
import { randomBytes } from "node:crypto";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const ide = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dataDir = process.env.EMERGENT_IDE_DATA_DIR?.trim() ? path.resolve(process.env.EMERGENT_IDE_DATA_DIR) : path.join(ide, ".data");
const endpoint = new URL("/api/mcp", process.env.EMERGENT_IDE_URL ?? "http://localhost:3000");
if (!["127.0.0.1", "localhost", "[::1]"].includes(endpoint.hostname)) { console.error("EMERGENT_IDE_URL must be a loopback address."); process.exit(1); }

async function token() {
  const file = path.join(dataDir, "mcp-token");
  try { const value = (await readFile(file, "utf8")).trim(); if (value.length >= 32) return value; } catch (error) { if (error?.code !== "ENOENT") throw error; }
  // Same contract as the IDE (src/lib/mcp-auth.ts): created once, owner-only, never overwritten.
  await mkdir(dataDir, { recursive: true, mode: 0o700 });
  try { await writeFile(file, `${randomBytes(32).toString("base64url")}\n`, { mode: 0o600, flag: "wx" }); await chmod(file, 0o600); } catch (error) { if (error?.code !== "EEXIST") throw error; }
  return (await readFile(file, "utf8")).trim();
}

const write = message => process.stdout.write(`${JSON.stringify(message)}\n`);
let pending = 0, closed = false;
const finish = () => { if (closed && pending === 0) process.exit(0); };

async function forward(line) {
  let message;
  try { message = JSON.parse(line); } catch { write({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Parse error" } }); return; }
  const ids = (Array.isArray(message) ? message : [message]).filter(item => item && item.id !== undefined && item.method).map(item => item.id);
  try {
    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream", Authorization: `Bearer ${await token()}` },
      body: JSON.stringify(message),
      signal: AbortSignal.timeout(180_000),
    });
    if (response.status === 202) return;
    const text = await response.text();
    if (!text) return;
    if ((response.headers.get("content-type") ?? "").includes("text/event-stream")) {
      for (const chunk of text.split(/\n\n/)) { const data = chunk.split("\n").filter(row => row.startsWith("data:")).map(row => row.slice(5).trim()).join(""); if (data) process.stdout.write(`${data}\n`); }
      return;
    }
    const body = JSON.parse(text);
    for (const item of Array.isArray(body) ? body : [body]) {
      if (item?.jsonrpc) write(item.id === null && ids.length === 1 ? { ...item, id: ids[0] } : item);
    }
  } catch (error) {
    const reason = error?.cause?.code === "ECONNREFUSED" ? `The Emergent IDE is not running at ${endpoint.origin}. Start it with \`pnpm dev\` in src/emergent-ide.` : `Emergent IDE request failed: ${error?.message ?? error}`;
    console.error(reason);
    for (const id of ids) write({ jsonrpc: "2.0", id, error: { code: -32000, message: reason } });
  }
}

createInterface({ input: process.stdin, crlfDelay: Infinity }).on("line", line => {
  if (!line.trim()) return;
  pending++;
  void forward(line).finally(() => { pending--; finish(); });
}).on("close", () => { closed = true; finish(); });
