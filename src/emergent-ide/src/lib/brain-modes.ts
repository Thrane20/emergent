// Brain component modes as the inspector edits them. The engine is the authority on validity
// (src/component.rs validate_modes); these helpers keep edits coherent: renames follow
// hand-over links, removals clear them, and plain lists stay in the legacy brain/stack form.
import type { ComponentPatch, ComponentValue, ModeValue } from "@/lib/components";

export type BrainMode = { name: string; brain: string; active: boolean; on_next?: string; on_previous?: string; keep_running?: boolean };

export const MAX_MODES = 16;
export const MODE_NAME = /^[a-z0-9_-]{1,32}$/;
const legacyName = (i: number) => (i === 0 ? "main" : `stack${i}`);

/** The component's modes, reading the legacy brain + stack as always-on modes main, stack1… */
export function modesFromFields(fields: Record<string, ComponentValue>): BrainMode[] {
  if (Array.isArray(fields.modes) && fields.modes.length > 0) {
    return (fields.modes as ModeValue[]).map(mode => {
      const text = (key: string) => (typeof mode[key] === "string" ? (mode[key] as string) : undefined);
      return { name: text("name") ?? "", brain: text("brain") ?? "", active: mode.active === true, on_next: text("on_next"), on_previous: text("on_previous"), keep_running: mode.keep_running === true };
    });
  }
  const paths = [typeof fields.brain === "string" ? fields.brain : "", ...(Array.isArray(fields.stack) ? (fields.stack as string[]) : [])].filter(Boolean);
  return paths.map((brain, i) => ({ name: legacyName(i), brain, active: true }));
}

/** Field changes for the component: the legacy form while nothing needs modes, otherwise `modes`. */
export function fieldsFromModes(modes: BrainMode[]): ComponentPatch {
  const plain = modes.length > 0 && modes.length <= 9 && modes.every((mode, i) => mode.name === legacyName(i) && mode.active && !mode.on_next && !mode.on_previous && !mode.keep_running);
  if (plain) return { brain: modes[0].brain, stack: modes.length > 1 ? modes.slice(1).map(mode => mode.brain) : null, modes: null };
  return {
    brain: null,
    stack: null,
    modes: modes.map(mode => {
      const value: ModeValue = { name: mode.name, brain: mode.brain };
      if (mode.active) value.active = true;
      if (mode.on_next) value.on_next = mode.on_next;
      if (mode.on_previous) value.on_previous = mode.on_previous;
      if (mode.keep_running) value.keep_running = true;
      return value;
    }),
  };
}

/** A problem the engine would reject, said in the inspector's words; empty when fine. */
export function modesProblem(modes: BrainMode[]): string {
  if (modes.length === 0) return "Add at least one mode.";
  if (modes.length > MAX_MODES) return `At most ${MAX_MODES} modes.`;
  const names = new Set<string>();
  for (const mode of modes) {
    if (!MODE_NAME.test(mode.name)) return `Mode name "${mode.name}" must be 1–32 characters of a–z, 0–9, _ and -.`;
    if (names.has(mode.name)) return `Two modes are named "${mode.name}".`;
    names.add(mode.name);
    if (!mode.brain) return `Choose a brain for mode "${mode.name}".`;
  }
  return "";
}

export function addMode(modes: BrainMode[], brain: string): BrainMode[] {
  let n = modes.length + 1;
  while (modes.some(mode => mode.name === `mode${n}`)) n++;
  return [...modes, { name: `mode${n}`, brain, active: modes.length === 0 }];
}

/** Renames a mode and every On next / On previous that names it. */
export function renameMode(modes: BrainMode[], index: number, name: string): BrainMode[] {
  const old = modes[index].name;
  return modes.map((mode, i) => ({
    ...mode,
    name: i === index ? name : mode.name,
    on_next: mode.on_next === old ? name : mode.on_next,
    on_previous: mode.on_previous === old ? name : mode.on_previous,
  }));
}

/** Removes a mode and clears hand-overs to it. */
export function removeMode(modes: BrainMode[], index: number): BrainMode[] {
  const gone = modes[index].name;
  return modes.filter((_, i) => i !== index).map(mode => ({
    ...mode,
    on_next: mode.on_next === gone ? undefined : mode.on_next,
    on_previous: mode.on_previous === gone ? undefined : mode.on_previous,
  }));
}

/** List order decides which mode steers first, so authors can move modes. */
export function moveMode(modes: BrainMode[], index: number, by: -1 | 1): BrainMode[] {
  const to = index + by;
  if (to < 0 || to >= modes.length) return modes;
  const next = [...modes];
  [next[index], next[to]] = [next[to], next[index]];
  return next;
}
