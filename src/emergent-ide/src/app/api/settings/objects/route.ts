import { objectError, sameOrigin } from "@/lib/object-http";
import { publicObjectSettings, updateObjectSettings } from "@/lib/object-settings";

export async function GET() {
  try { return Response.json(await publicObjectSettings()); }
  catch (error) { return objectError(error); }
}

export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    return Response.json(await updateObjectSettings(await request.json()));
  } catch (error) { return objectError(error); }
}
