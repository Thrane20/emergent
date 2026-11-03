import assert from "node:assert/strict";
import test from "node:test";
import { aiBalances } from "../src/lib/ai-balances";
import type { AIConnectionSettings } from "../src/lib/ai-settings";

const connections: AIConnectionSettings[] = [
  { provider: "anthropic", model: "test", baseUrl: "https://api.anthropic.com", apiKey: "anthropic-secret" },
  { provider: "openai-codex", model: "test", baseUrl: "https://api.openai.com/v1", apiKey: "openai-secret" },
  { provider: "vercel", model: "creator/model", baseUrl: "https://ai-gateway.vercel.sh/v1", apiKey: "gateway-secret" },
];
const credentials = async () => connections;

test("configuration discovery makes no provider calls; unsupported balances never become zero", async () => {
  const result = await aiBalances(false, credentials, async () => { throw new Error("Must not call a provider"); });
  assert.deepEqual(result.map(item => item.state), ["unsupported", "unsupported", "unchecked"]);
  assert.ok(result.every(item => item.balance === null && item.checkedAt === null));
  assert.ok(!JSON.stringify(result).includes("secret"));
  assert.deepEqual(await aiBalances(false, async () => []), []);
});

test("only explicit refresh calls Vercel, with a bounded authenticated read and private account fields omitted", async () => {
  let calls = 0;
  const result = await aiBalances(true, credentials, async (input, init) => {
    calls++;
    assert.equal(input, "https://ai-gateway.vercel.sh/v1/credits");
    assert.equal(new Headers(init?.headers).get("Authorization"), "Bearer gateway-secret");
    assert.equal(init?.cache, "no-store"); assert.equal(init?.redirect, "error"); assert.ok(init?.signal);
    return Response.json({ balance: "4.9940441", total_used: "private-usage", account: "private-account" });
  });
  assert.equal(calls, 1);
  assert.deepEqual(result.map(item => item.state), ["unsupported", "unsupported", "ready"]);
  assert.equal(result[2].balance, 4.9940441);
  assert.ok(result[2].checkedAt);
  assert.doesNotMatch(JSON.stringify(result), /secret|private-usage|private-account/);
});

test("errors, malformed balances, redirects and custom URLs never fabricate or disclose a balance", async () => {
  const vercel = async () => [connections[2]];
  for (const status of [401, 403, 429, 500]) {
    const [result] = await aiBalances(true, vercel, async () => new Response("gateway-secret", { status }));
    assert.equal(result.state, "unavailable"); assert.equal(result.balance, null); assert.doesNotMatch(result.message, /gateway-secret/);
  }
  for (const balance of [undefined, null, "", "NaN", "Infinity", "1oops", true, {}, []]) {
    assert.equal((await aiBalances(true, vercel, async () => Response.json({ balance })))[0].state, "unavailable");
  }
  for (const balance of [0, "0", "-0.25", 123.45]) {
    const [result] = await aiBalances(true, vercel, async () => Response.json({ balance }));
    assert.equal(result.state, "ready"); assert.equal(result.balance, Number(balance));
  }
  assert.equal((await aiBalances(true, vercel, async () => { throw new Error("gateway-secret"); }))[0].state, "unavailable");
  assert.equal((await aiBalances(true, vercel, async () => new Response("x".repeat(32_769))))[0].state, "unavailable");
  const [custom] = await aiBalances(true, async () => [{ ...connections[2], baseUrl: "https://custom.example/v1" }], async () => { throw new Error("Must not send this key"); });
  assert.equal(custom.state, "unsupported");
});
