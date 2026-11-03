import { vercelModels } from "@/lib/vercel-models";

export async function GET() {
  try {
    return Response.json({ models: await vercelModels() });
  } catch {
    return Response.json({ error: "Unable to load Vercel models. Try again or enter a creator/model ID." }, { status: 502 });
  }
}
