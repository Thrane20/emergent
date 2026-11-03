import { aiBalances } from "@/lib/ai-balances";

async function response(refresh: boolean) {
  const headers = { "Cache-Control": "private, no-store" };
  try {
    return Response.json({ balances: await aiBalances(refresh) }, { headers });
  } catch {
    return Response.json({ error: "Unable to read AI balance settings." }, { status: 500, headers });
  }
}

export async function GET() { return response(false); }
export async function POST() { return response(true); }
