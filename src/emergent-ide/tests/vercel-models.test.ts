import assert from "node:assert/strict";
import test from "node:test";
import { vercelModels } from "../src/lib/vercel-models";

test("discovery uses the public catalogue without credentials and returns sorted tool/structured language models", async () => {
  const models = await vercelModels(async (url, init) => {
    assert.equal(url, "https://ai-gateway.vercel.sh/v1/models");
    assert.equal(init?.headers, undefined);
    assert.equal(init?.cache, "no-store");
    assert.ok(init?.signal);
    return Response.json({ data: [
      { id: "z/chat", name: "Z", type: "language", tags: ["tool-use", "structured-output"] },
      { id: "a/chat", name: "A", type: "language", tags: ["tool-use", "structured-output"] },
      { id: "a/text-only", name: "Text", type: "language", tags: [] },
      { id: "a/tools-only", name: "Tools", type: "language", tags: ["tool-use"] },
      { id: "a/image", name: "Image", type: "image", tags: ["tool-use", "structured-output"] },
    ] });
  });
  assert.deepEqual(models, [{ id: "a/chat", name: "A" }, { id: "z/chat", name: "Z" }]);
});

test("catalogue errors and invalid payloads fail rather than returning misleading choices", async () => {
  await assert.rejects(vercelModels(async () => new Response("unavailable", { status: 503 })), /unavailable \(503\)/);
  await assert.rejects(vercelModels(async () => Response.json({ data: [{ id: 123 }] })));
  await assert.rejects(vercelModels(async () => { throw new Error("network unavailable"); }), /network unavailable/);
});
