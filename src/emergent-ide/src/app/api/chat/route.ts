import { validateChatRequest, type ChatRequest } from "@/lib/ai-chat";
import { implementChat } from "@/lib/chat-implementation";
import { objectProject, sameOrigin } from "@/lib/object-http";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const input = validateChatRequest(await request.json() as ChatRequest);
    return Response.json(await implementChat(await objectProject(input.projectId), input));
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Unable to complete the chat request." }, { status: 400 });
  }
}
