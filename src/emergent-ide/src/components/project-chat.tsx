"use client";

import { FormEvent, KeyboardEvent, useMemo, useRef, useState } from "react";
import Link from "next/link";
import type { PublicAISettings } from "@/lib/ai-settings";
import type { ProjectFile } from "@/lib/project";

type ChatMessage = {
  id: number;
  role: "user" | "assistant";
  text: string;
  state?: "pending" | "error";
};

type Props = {
  projectId: string;
  selectedFile: ProjectFile;
  aiSettings: PublicAISettings;
};

export function ProjectChat({ projectId, selectedFile, aiSettings }: Props) {
  const [draft, setDraft] = useState("");
  const [expanded, setExpanded] = useState(true);
  const [messages, setMessages] = useState<Record<string, ChatMessage[]>>({});
  const [sending, setSending] = useState(false);
  const nextMessageId = useRef(0);
  const activeScope = `file:${selectedFile.path}`;
  const thread = messages[activeScope] ?? [];
  const scopeLabel = selectedFile.name;
  const definitions = useMemo(() => ["emergent.toml", selectedFile.path], [selectedFile.path]);

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    const text = draft.trim();
    if (!text || sending) return;
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
        { id: assistantMessageId, role: "assistant", text: "Thinking with the linked project graph…", state: "pending" },
      ],
    }));
    setDraft("");
    setSending(true);
    try {
      const response = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          projectId,
          scope: requestScope,
          message: text,
          history: requestHistory.map(({ role, text: historyText }) => ({ role, text: historyText })),
        }),
      });
      const result = await response.json() as { text?: string; error?: string };
      if (!response.ok || !result.text) throw new Error(result.error ?? "The AI provider returned no response.");
      replacePending(requestScope, assistantMessageId, { id: assistantMessageId, role: "assistant", text: result.text });
    } catch (error) {
      replacePending(requestScope, assistantMessageId, {
        id: assistantMessageId,
        role: "assistant",
        text: error instanceof Error ? error.message : "Unable to reach the AI provider.",
        state: "error",
      });
    } finally {
      setSending(false);
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
        <Link className={aiSettings.configured ? "provider-status connected" : "provider-status"} href="/settings" title="Open AI settings"><i />{aiSettings.configured ? `Codex · ${aiSettings.model}` : "AI not connected"}</Link>
        <button className="chat-collapse" onClick={() => setExpanded((value) => !value)} aria-label={expanded ? "Collapse chat" : "Expand chat"}>{expanded ? "⌄" : "⌃"}</button>
      </header>
      {expanded && <>
        <div className="chat-context">
          <div className="chat-bound-scope"><span>Scope</span><b>{selectedFile.name}</b></div>
          <div className="definition-links"><span>Context</span>{definitions.map((definition) => <b key={definition}>{definition}</b>)}</div>
        </div>
        <div className="chat-messages">
          {thread.length === 0
            ? <div className="chat-empty"><span>Ask for a component, behaviour, scene change, or new project file.</span><small>The selected scope and definitions will travel with the request.</small></div>
            : thread.map((message) => <div className={`chat-message ${message.role} ${message.state ?? ""}`} key={message.id}><b>{message.role === "user" ? "You" : "Emergent"}</b><p>{message.text}</p></div>)}
        </div>
        <form className="chat-composer" onSubmit={submit}>
          <textarea value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={onKeyDown} placeholder={`Describe what to build for ${scopeLabel}…`} rows={2} />
          <button type="submit" disabled={!draft.trim() || sending} title="Send request to Codex">{sending ? "…" : "↑"}</button>
        </form>
      </>}
    </section>
  );
}
