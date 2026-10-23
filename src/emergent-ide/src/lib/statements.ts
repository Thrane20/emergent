import { randomUUID } from "node:crypto";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import "server-only";
import type { ProjectSnapshot } from "@/lib/project";

export type StatementConfidence = "low" | "medium" | "high";

export type ProjectStatement = {
  id: string;
  subject: string;
  text: string;
  implemented: boolean | null;
  confidence: StatementConfidence | null;
  evidence: string | null;
  checkedAt: string | null;
  implementation?: { summary: string; reused: string[]; files: string[]; validatedAt: string } | null;
};

type StatementDocument = {
  schema_version: 1;
  statements: ProjectStatement[];
};

let writeQueue: Promise<void> = Promise.resolve();

export async function statementsFor(project: ProjectSnapshot, subject: string): Promise<ProjectStatement[]> {
  validateSubject(project, subject);
  const document = await readDocument(project.root);
  return document.statements.filter((statement) => statement.subject === subject);
}

export async function addStatement(project: ProjectSnapshot, subject: string, text: string): Promise<ProjectStatement[]> {
  validateSubject(project, subject);
  const normalized = validateText(text);
  return updateDocument(project.root, (document) => {
    document.statements.push({
      id: randomUUID(),
      subject,
      text: normalized,
      implemented: null,
      confidence: null,
      evidence: null,
      checkedAt: null,
    });
    return document;
  }).then((document) => document.statements.filter((statement) => statement.subject === subject));
}

export async function removeStatement(project: ProjectSnapshot, subject: string, id: string): Promise<ProjectStatement[]> {
  validateSubject(project, subject);
  return updateDocument(project.root, (document) => {
    const before = document.statements.length;
    document.statements = document.statements.filter((statement) => !(statement.subject === subject && statement.id === id));
    if (document.statements.length === before) throw new Error("Statement was not found for the selected subject.");
    return document;
  }).then((document) => document.statements.filter((statement) => statement.subject === subject));
}

export async function statementById(project: ProjectSnapshot, subject: string, id: string): Promise<ProjectStatement> {
  const statements = await statementsFor(project, subject);
  const statement = statements.find((candidate) => candidate.id === id);
  if (!statement) throw new Error("Statement was not found for the selected subject.");
  return statement;
}

export async function saveStatementEvaluation(
  project: ProjectSnapshot,
  subject: string,
  id: string,
  evaluation: { implemented: boolean; confidence: StatementConfidence; evidence: string },
): Promise<ProjectStatement[]> {
  validateSubject(project, subject);
  const evidence = validateEvidence(evaluation.evidence);
  return updateDocument(project.root, (document) => {
    const statement = document.statements.find((candidate) => candidate.subject === subject && candidate.id === id);
    if (!statement) throw new Error("Statement was not found for the selected subject.");
    statement.implemented = evaluation.implemented;
    statement.confidence = evaluation.confidence;
    statement.evidence = evidence;
    statement.checkedAt = new Date().toISOString();
    return document;
  }).then((document) => document.statements.filter((statement) => statement.subject === subject));
}

export async function saveStatementImplementation(project: ProjectSnapshot, subject: string, id: string, implementation: NonNullable<ProjectStatement["implementation"]>) {
  validateSubject(project, subject);
  await updateDocument(project.root, (document) => {
    const statement = document.statements.find((item) => item.subject === subject && item.id === id);
    if (!statement) throw new Error("Statement was removed during implementation.");
    statement.implementation = implementation;
    statement.implemented = null; statement.confidence = null; statement.evidence = null; statement.checkedAt = null;
    return document;
  });
}

export function validateSubject(project: ProjectSnapshot, subject: string): void {
  if (!subject.startsWith("file:")) throw new Error("This statement subject type is not supported yet.");
  const requestedPath = subject.slice("file:".length);
  if (!project.files.some((file) => file.path === requestedPath)) {
    throw new Error("The statement subject is not present in the linked project graph.");
  }
}

async function updateDocument(root: string, update: (document: StatementDocument) => StatementDocument): Promise<StatementDocument> {
  let result: StatementDocument | undefined;
  const operation = writeQueue.then(async () => {
    const document = update(await readDocument(root));
    const target = path.join(root, "emergent.statements.json");
    const temporary = path.join(root, `.emergent.statements.${process.pid}.tmp`);
    await writeFile(temporary, `${JSON.stringify(document, null, 2)}\n`, { mode: 0o644 });
    await rename(temporary, target);
    result = document;
  });
  writeQueue = operation.catch(() => undefined);
  await operation;
  if (!result) throw new Error("Statement update did not complete.");
  return result;
}

async function readDocument(root: string): Promise<StatementDocument> {
  try {
    const parsed = JSON.parse(await readFile(path.join(root, "emergent.statements.json"), "utf8")) as Partial<StatementDocument>;
    if (parsed.schema_version !== 1 || !Array.isArray(parsed.statements)) {
      throw new Error("emergent.statements.json has an unsupported schema.");
    }
    return { schema_version: 1, statements: parsed.statements.map(validateStoredStatement) };
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return { schema_version: 1, statements: [] };
    }
    throw error;
  }
}

function validateStoredStatement(value: unknown): ProjectStatement {
  if (!value || typeof value !== "object") throw new Error("Stored statement is invalid.");
  const statement = value as Partial<ProjectStatement>;
  if (typeof statement.id !== "string" || typeof statement.subject !== "string") throw new Error("Stored statement identity is invalid.");
  const implemented = statement.implemented === true || statement.implemented === false ? statement.implemented : null;
  const confidence = statement.confidence === "low" || statement.confidence === "medium" || statement.confidence === "high" ? statement.confidence : null;
  return {
    id: statement.id,
    subject: statement.subject,
    text: validateText(statement.text),
    implemented,
    confidence,
    evidence: typeof statement.evidence === "string" ? validateEvidence(statement.evidence) : null,
    checkedAt: typeof statement.checkedAt === "string" ? statement.checkedAt : null,
    implementation: statement.implementation && typeof statement.implementation.summary === "string" && typeof statement.implementation.validatedAt === "string" && Array.isArray(statement.implementation.reused) && statement.implementation.reused.every((item) => typeof item === "string") && Array.isArray(statement.implementation.files) && statement.implementation.files.every((item) => typeof item === "string") ? statement.implementation : null,
  };
}

function validateText(value: unknown): string {
  if (typeof value !== "string" || !value.trim()) throw new Error("Statement text is required.");
  const text = value.trim();
  if (text.length > 1_000) throw new Error("Statement text is too long.");
  return text;
}

function validateEvidence(value: unknown): string {
  if (typeof value !== "string" || !value.trim()) throw new Error("AI evidence is required.");
  const evidence = value.trim();
  if (evidence.length > 1_000) throw new Error("AI evidence is too long.");
  return evidence;
}
