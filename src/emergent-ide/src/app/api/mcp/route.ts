import { WebStandardStreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/webStandardStreamableHttp.js";
import { mcpRejection } from "@/lib/mcp-auth";
import { createMcpServer } from "@/lib/mcp-server";

// Stateless Streamable HTTP: one server and transport per request, plain JSON responses.
export async function POST(request: Request) {
  const rejection = await mcpRejection(request);
  if (rejection) return rejection;
  const server = createMcpServer();
  const transport = new WebStandardStreamableHTTPServerTransport({ sessionIdGenerator: undefined, enableJsonResponse: true });
  await server.connect(transport);
  try { return await transport.handleRequest(request); }
  finally { void server.close(); }
}

export async function GET() {
  return new Response(null, { status: 405, headers: { Allow: "POST" } });
}
