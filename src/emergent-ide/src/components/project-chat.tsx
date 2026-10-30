"use client";

import { FormEvent, KeyboardEvent, useRef, useState } from "react";
import Link from "next/link";
import type { PublicAISettings } from "@/lib/ai-settings";

type ChatMessage = {
  id: number;
  role: "user" | "assistant";
  text: string;
  state?: "pending" | "error";
};

type Props = {
  projectId: string;
  projectName: string;
  scene?: { path: string; name: string };
  object?: { scope: string; name: string };
  aiSettings: PublicAISettings;
  disabled: boolean;
  onBusy: (busy:boolean)=>void;
  onApplied: ()=>void;
};

export function ProjectChat({ projectId, projectName, scene, object, aiSettings, disabled, onBusy, onApplied }: Props) {
  const [scopeKind, setScopeKind] = useState<"game" | "scene" | "object">("object");
  const [drafts, setDrafts] = useState<Record<string,string>>({});
  const [expanded, setExpanded] = useState(true);
  const [messages, setMessages] = useState<Record<string, ChatMessage[]>>({});
  const [sending, setSending] = useState(false);
  const nextMessageId = useRef(0);
  const kind = scopeKind === "object" && !object ? (scene ? "scene" : "game") : scopeKind === "scene" && !scene ? "game" : scopeKind;
  const activeScope = kind === "object" && object ? object.scope : kind === "scene" && scene ? `file:${scene.path}` : `project:${projectId}`;
  const scopeLabel = kind === "object" && object ? object.name : kind === "scene" && scene ? scene.name : projectName;
  const thread = messages[activeScope] ?? [];
  const draft = drafts[activeScope] ?? "";
  const setDraft = (text:string) => setDrafts(current=>({...current,[activeScope]:text}));

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    const text = draft.trim();
    if (!text || sending || disabled) return;
    if (!aiSettings.configured) {
      const messageId = ++nextMessageId.current;
      setMessages((current) => ({
        ...current,
        [activeScope]: [...(current[activeScope] ?? []), { id: messageId, role: "assistant", text: "Configure an API key in AI settings before sending a message.", state: "error" }],
      }));
      return;
    }
    const requestScope = activeScope;
    const requestHistory = thread.filter((message) => message.state !== "pending");
    const userMessageId = ++nextMessageId.current;
    const assistantMessageId = ++nextMessageId.current;
    setMessages((current) => ({
      ...current,
      [requestScope]: [
        ...(current[requestScope] ?? []),
        { id: userMessageId, role: "user", text },
        { id: assistantMessageId, role: "assistant", text: "Inspecting the project and preparing changes…", state: "pending" },
      ],
    }));
    setDraft("");
    setSending(true);
    onBusy(true);
    try {
      const response = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          projectId,
          scope: requestScope,
          scenePath: scene?.path,
          message: text,
          history: requestHistory.map(({ role, text: historyText }) => ({ role, text: historyText })),
        }),
      });
      const result = await response.json() as { text?: string; error?: string; applied?: boolean; files?: string[] };
      if (!response.ok || !result.text) throw new Error(result.error ?? "The AI provider returned no response.");
      replacePending(requestScope, assistantMessageId, { id: assistantMessageId, role: "assistant", text: result.text });
      if (result.applied) onApplied();
    } catch (error) {
      replacePending(requestScope, assistantMessageId, {
        id: assistantMessageId,
        role: "assistant",
        text: error instanceof Error ? error.message : "Unable to reach the AI provider.",
        state: "error",
      });
    } finally {
      setSending(false);
      onBusy(false);
    }
  };

  const replacePending = (requestScope: string, id: number, replacement: ChatMessage) => {
    setMessages((current) => ({
      ...current,
      [requestScope]: (current[requestScope] ?? []).map((message) => message.id === id ? replacement : message),
    }));
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
        <Link className={aiSettings.configured ? "provider-status connected" : "provider-status"} href="/settings" title="Open AI settings"><i />{aiSettings.configured ? `${aiSettings.label} · ${aiSettings.model}` : "AI not connected"}</Link>
        <button className="chat-collapse" onClick={() => setExpanded((value) => !value)} aria-label={expanded ? "Collapse chat" : "Expand chat"}>{expanded ? "⌄" : "⌃"}</button>
      </header>
      {expanded && <>
        <div className="chat-context">
          <label className="chat-bound-scope"><span>Scope</span><select aria-label="AI scope" value={kind} onChange={event=>setScopeKind(event.target.value as "game"|"scene"|"object")}>
            <option value="game">Game</option><option value="scene" disabled={!scene}>Scene</option><option value="object" disabled={!object}>Object</option>
          </select></label>
        </div>
        <div className="chat-messages">
          {thread.length === 0
            ? null
            : thread.map((message) => <div className={`chat-message ${message.role} ${message.state ?? ""}`} key={message.id}><b>{message.role === "user" ? "You" : "Emergent"}</b><p>{message.text}</p></div>)}
        </div>
        <form className="chat-composer" onSubmit={submit}>
          <textarea disabled={disabled || sending} value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={onKeyDown} placeholder={`Describe what to build for ${scopeLabel}…`} rows={2} />
          <button type="submit" disabled={!draft.trim() || sending || disabled} title="Send request to Codex">{sending ? "…" : "↑"}</button>
        </form>
      </>}
    </section>
  );
}
