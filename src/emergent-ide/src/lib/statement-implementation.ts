import "server-only";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { projectCodeContext } from "./project-code-context";
import { implementationFormat, implementationPrompt, parseImplementation } from "./statement-proposal";
import { applyChanges, checkCandidate, prepareChanges } from "./statement-files";
import { evaluateStatement, statementResponse } from "./statement-ai";
import { saveStatementImplementation, statementById, statementsFor } from "./statements";

const state = globalThis as typeof globalThis & { emergentStatementImplementation?: boolean };

export async function implementStatement(project: ProjectSnapshot, subject: string, id: string) {
  if (state.emergentStatementImplementation) throw new Error("Another statement is being implemented. Wait for it to finish.");
  state.emergentStatementImplementation = true;
  try {
    const statement = await statementById(project, subject, id);
    const context = await projectCodeContext(project, statement);
    const proposal = parseImplementation(await statementResponse(implementationPrompt(project, statement, context), implementationFormat, 14_000), context);
    const changes = await prepareChanges(project, proposal, context);
    if (changes.length) {
      await checkCandidate(project, changes, context);
      // A removed statement cannot authorize publishing a now-unwanted change.
      await statementById(project, subject, id);
      await applyChanges(project, changes);
      await saveStatementImplementation(project, subject, id, {
        summary: proposal.summary,
        reused: proposal.reuse.map((item) => `${item.path} · ${item.symbol}: ${item.reason}`),
        files: changes.map((file) => file.path), validatedAt: new Date().toISOString(),
      });
    }
    const fresh = await readProjectSnapshot();
    if (fresh.id !== project.id || fresh.root !== project.root) throw new Error("The linked project changed during implementation.");
    try { return { statements: await evaluateStatement(fresh, subject, id), warning: changes.length ? undefined : `No file changes: ${proposal.summary}` }; }
    catch (error) { return { statements: await statementsFor(fresh, subject), warning: `Implementation completed, but its follow-up check failed: ${error instanceof Error ? error.message : "unknown error"}` }; }
  } finally { state.emergentStatementImplementation = false; }
}
