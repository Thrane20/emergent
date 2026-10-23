import { readPublicAISettings, updateAISettings, type AISettingsUpdate } from "@/lib/ai-settings";

export async function GET() {
  return Response.json(await readPublicAISettings());
}

export async function PUT(request: Request) {
  try {
    const input = await request.json() as AISettingsUpdate;
    return Response.json(await updateAISettings(input));
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unable to save AI settings.";
    return Response.json({ error: message }, { status: 400 });
  }
}
