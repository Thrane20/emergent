"use client";

import { FormEvent, KeyboardEvent, useMemo, useState } from "react";
import Link from "next/link";
import type { PublicAISettings } from "@/lib/ai-settings";
import type { ProjectEntity, ProjectFile } from "@/lib/project";

type ChatMessage = {
  id: number;
  role: "user" | "assistant";
  text: string;
};

type Props = {
  projectId: string;
  projectName: string;
  selectedFile: ProjectFile;
  entities: ProjectEntity[];
  aiSettings: PublicAISettings;
};

export function ProjectChat({ projectId, projectName, selectedFile, entities, aiSettings }: Props) {
  const projectScope = `project:${projectId}`;
  const fileScope = `file:${selectedFile.path}`;
  const [scope, setScope] = useState(projectScope);
  const [draft, setDraft] = useState("");
  const [expanded, setExpanded] = useState(true);
  const [messages, setMessages] = useState<Record<string, ChatMessage[]>>({});
  const activeScope = scope.startsWith("file:") ? fileScope : scope;
  const thread = messages[activeScope] ?? [];
  const linkedEntity = entities.find((entity) => entityScope(entity) === activeScope);
  const scopeLabel = activeScope === projectScope
    ? projectName
    : activeScope.startsWith("file:")
      ? selectedFile.name
      : linkedEntity?.id ?? "Entity";
  const definitions = useMemo(() => {
    if (linkedEntity) return ["emergent.toml", linkedEntity.sourcePath, `entity:${linkedEntity.id}`];
    if (activeScope.startsWith("file:")) return ["emergent.toml", selectedFile.path];
    return ["emergent.toml", "project graph"];
  }, [activeScope, linkedEntity, selectedFile.path]);

  const submit = (event?: FormEvent) => {
    event?.preventDefault();
    const text = draft.trim();
    if (!text) return;
    const now = Date.now();
    setMessages((current) => ({
      ...current,
      [activeScope]: [
        ...(current[activeScope] ?? []),
        { id: now, role: "user", text },
        {
          id: now + 1,
          role: "assistant",
          text: aiSettings.configured
            ? "Request staged with this scope. Codex is configured; request execution and reviewed file changes are the next integration step."
            : "Request staged with this scope. Connect an AI provider before code or project files can be generated.",
        },
      ],
    }));
    setDraft("");
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  return (
    <section className={expanded ? "project-chat expanded" : "project-chat"} aria-label="Project AI chat">
      <header className="chat-header">
        <button className="chat-title" onClick={() => setExpanded((value) => !value)}>
          <span className="chat-spark">✦</span>
          <span><b>Build with AI</b><small>{scopeLabel}</small></span>
        </button>
        <Link className={aiSettings.configured ? "provider-status connected" : "provider-status"} href="/settings" title="Open AI settings"><i />{aiSettings.configured ? `Codex · ${aiSettings.model}` : "AI not connected"}</Link>
        <button className="chat-collapse" onClick={() => setExpanded((value) => !value)} aria-label={expanded ? "Collapse chat" : "Expand chat"}>{expanded ? "⌄" : "⌃"}</button>
      </header>
      {expanded && <>
        <div className="chat-context">
          <label>Scope<select value={activeScope} onChange={(event) => setScope(event.target.value)}>
            <option value={projectScope}>Project · {projectName}</option>
            <option value={fileScope}>Selected file · {selectedFile.name}</option>
            <optgroup label="Authored entities">{entities.map((entity) => <option value={entityScope(entity)} key={entityScope(entity)}>Entity · {entity.id} ({entity.kind})</option>)}</optgroup>
          </select></label>
          <div className="definition-links"><span>Context</span>{definitions.map((definition) => <b key={definition}>{definition}</b>)}</div>
        </div>
        <div className="chat-messages">
          {thread.length === 0
            ? <div className="chat-empty"><span>Ask for a component, behaviour, scene change, or new project file.</span><small>The selected scope and definitions will travel with the request.</small></div>
            : thread.map((message) => <div className={`chat-message ${message.role}`} key={message.id}><b>{message.role === "user" ? "You" : "Emergent"}</b><p>{message.text}</p></div>)}
        </div>
        <form className="chat-composer" onSubmit={submit}>
          <textarea value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={onKeyDown} placeholder={`Describe what to build for ${scopeLabel}…`} rows={2} />
          <button type="submit" disabled={!draft.trim()} title="Stage request">↑</button>
        </form>
      </>}
    </section>
  );
}

function entityScope(entity: ProjectEntity): string {
  return `entity:${entity.sourcePath}#${entity.id}`;
}
