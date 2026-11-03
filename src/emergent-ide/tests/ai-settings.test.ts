import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

let data = "";
let settings: typeof import("../src/lib/ai-settings");
test.before(async () => {
  data = await mkdtemp(path.join(os.tmpdir(), "emergent-ai-settings-"));
  process.env.EMERGENT_IDE_DATA_DIR = data;
  settings = await import("../src/lib/ai-settings");
});
test.after(() => rm(data, { recursive: true, force: true }));
const readPublicAISettings = () => settings.readPublicAISettings();
const readAIConnectionSettings = () => settings.readAIConnectionSettings();
const updateAISettings = (input: Parameters<typeof settings.updateAISettings>[0]) => settings.updateAISettings(input);

test("a fresh install defaults to Claude with no key", async () => {
  const fresh = await readPublicAISettings();
  assert.equal(fresh.provider, "anthropic");
  assert.equal(fresh.model, "claude-opus-5-5");
  assert.equal(fresh.configured, false);
  assert.equal(fresh.providers.vercel.configured, false);
  assert.equal(fresh.providers.vercel.baseUrl, "https://ai-gateway.vercel.sh/v1");
});

test("Vercel keeps an isolated private key, validates model IDs and supports model changes and key removal", async () => {
  const before = await readPublicAISettings();
  await updateAISettings({ provider: "vercel", model: "openai/gpt-6.1-sol", apiKey: "gateway-test-private-key" });
  assert.deepEqual(await readAIConnectionSettings(), {
    provider: "vercel", baseUrl: "https://ai-gateway.vercel.sh/v1", model: "openai/gpt-6.1-sol", apiKey: "gateway-test-private-key",
  });
  const visible = await readPublicAISettings();
  assert.ok(!JSON.stringify(visible).includes("gateway-test-private-key"));
  assert.deepEqual(visible.providers.anthropic, before.providers.anthropic);
  assert.deepEqual(visible.providers["openai-codex"], before.providers["openai-codex"]);
  assert.equal((await stat(path.join(data, "ai-provider.json"))).mode & 0o777, 0o600);
  assert.ok((await settings.readAIProviderConnections()).some(connection => connection.provider === "vercel" && connection.apiKey === "gateway-test-private-key"));
  await updateAISettings({ provider: "vercel", model: "anthropic/claude-sonnet-4.6" });
  assert.equal((await readAIConnectionSettings()).apiKey, "gateway-test-private-key");
  await assert.rejects(updateAISettings({ provider: "vercel", model: "no-creator" }), /creator\/model/);
  await updateAISettings({ provider: "anthropic" });
  assert.equal((await readPublicAISettings()).providers.vercel.model, "anthropic/claude-sonnet-4.6");
  await updateAISettings({ provider: "vercel", clearApiKey: true });
  assert.equal((await readAIConnectionSettings()).apiKey, "");
  assert.ok(!(await settings.readAIProviderConnections()).some(connection => connection.provider === "vercel"));
});

test("legacy single-provider files migrate to Codex and keep their key and model", async () => {
  await writeFile(path.join(data, "ai-provider.json"), JSON.stringify({ provider: "openai-codex", baseUrl: "https://api.openai.com/v1", model: "gpt-5.6-sol", apiKey: "sk-legacy-123456" }));
  const connection = await readAIConnectionSettings();
  assert.deepEqual(connection, { provider: "openai-codex", baseUrl: "https://api.openai.com/v1", model: "gpt-5.6-sol", apiKey: "sk-legacy-123456" });
  const visible = await readPublicAISettings();
  assert.equal(visible.providers.anthropic.configured, false);
  assert.ok(!JSON.stringify(visible).includes("sk-legacy-123456"));
});

test("switching providers keeps each provider's own key and model", async () => {
  await updateAISettings({ provider: "anthropic", model: "claude-sonnet-5-5", apiKey: "sk-ant-abcdefgh" });
  let connection = await readAIConnectionSettings();
  assert.deepEqual(connection, { provider: "anthropic", baseUrl: "https://api.anthropic.com", model: "claude-sonnet-5-5", apiKey: "sk-ant-abcdefgh" });
  await updateAISettings({ provider: "openai-codex", model: "gpt-5.6-sol" });
  connection = await readAIConnectionSettings();
  assert.equal(connection.apiKey, "sk-legacy-123456");
  const stored = JSON.parse(await readFile(path.join(data, "ai-provider.json"), "utf8"));
  assert.equal(stored.providers.anthropic.apiKey, "sk-ant-abcdefgh");
  await assert.rejects(updateAISettings({ provider: "gemini" }), /Claude, Codex, Vercel AI Gateway or Ollama/);
  await updateAISettings({ provider: "anthropic", clearApiKey: true, model: "claude-opus-5-5" });
  assert.equal((await readPublicAISettings()).providers.anthropic.configured, false);
  assert.equal((await readPublicAISettings()).providers["openai-codex"].configured, true);
});

test("Ollama activates without a key, stays local and can be deactivated", async () => {
  let saved = await updateAISettings({ provider: "ollama", baseUrl: "http://localhost:11500/v1", model: "qwen3-coder:30b" });
  assert.equal(saved.configured, true);
  assert.equal(saved.keyHint, null);
  assert.deepEqual(await readAIConnectionSettings(), { provider: "ollama", baseUrl: "http://localhost:11500/v1", model: "qwen3-coder:30b", apiKey: "ollama" });
  await assert.rejects(updateAISettings({ provider: "ollama", baseUrl: "https://example.com/v1" }), /localhost/);
  saved = await updateAISettings({ provider: "ollama", baseUrl: "http://localhost:11500/v1", model: "qwen3-coder:30b", clearApiKey: true });
  assert.equal(saved.configured, false);
});
