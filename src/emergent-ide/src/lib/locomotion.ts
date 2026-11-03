// Locomotion profiles (`*.locomotion.toml`): a type from the engine's catalogue plus its tuning fields.
import { parse, stringify } from "smol-toml";
import type { ComponentField, ComponentValue } from "./components";

export type LocomotionType = { id: string; label: string; category: string; status: "available" | "boundary" | "planned"; description: string; manoeuvres: string[]; rig: "body" | "wheels" | "legs" | "rotors" | "wing" | "projectile"; fields: ComponentField[] };
export type LocomotionCatalogue = { categories: string[]; types: LocomotionType[] };
export type LocomotionProfile = { id: string; name: string; description: string; type: string; fields: Record<string, ComponentValue> };
export type LocomotionSummary = { path: string; id: string; name: string; description: string; type: string; error?: string };

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);

export function locomotionCatalogue(exported: unknown): LocomotionCatalogue {
  if (!object(exported) || !Array.isArray(exported.categories) || !Array.isArray(exported.types)) throw new Error("Invalid locomotion catalogue.");
  const types = exported.types.map(type => {
    if (!object(type) || typeof type.id !== "string" || typeof type.label !== "string" || !Array.isArray(type.fields ?? [])) throw new Error("Invalid locomotion type.");
    return { ...type, fields: Array.isArray(type.fields) ? type.fields : [], manoeuvres: Array.isArray(type.manoeuvres) ? type.manoeuvres : [] } as LocomotionType;
  });
  return { categories: exported.categories.filter((c): c is string => typeof c === "string"), types };
}

export function parseProfile(source: string): LocomotionProfile {
  const document = parse(source) as Record<string, unknown>;
  if (document.schema_version !== 1 || typeof document.id !== "string" || typeof document.type !== "string") throw new Error("A locomotion profile needs schema_version = 1, an id and a type.");
  const { id, name, description, type } = document;
  const reserved = new Set(["schema_version", "id", "name", "kind", "description", "type", "template", "runtime_status"]);
  const fields = Object.fromEntries(Object.entries(document).filter(([key]) => !reserved.has(key)));
  return { id: id as string, name: typeof name === "string" ? name : id as string, description: typeof description === "string" ? description : "", type: type as string, fields: JSON.parse(JSON.stringify(fields)) as Record<string, ComponentValue> };
}

export function serializeProfile(profile: LocomotionProfile): string {
  return stringify({ schema_version: 1, id: profile.id, name: profile.name, kind: "locomotion", ...(profile.description.trim() ? { description: profile.description.trim() } : {}), type: profile.type, ...profile.fields });
}

/** Fields the new type does not have are dropped when switching type. */
export function changeType(profile: LocomotionProfile, type: LocomotionType): LocomotionProfile {
  const keep = new Set(type.fields.map(field => field.name));
  return { ...profile, type: type.id, fields: Object.fromEntries(Object.entries(profile.fields).filter(([name]) => keep.has(name))) };
}

export type LocomotionFrame = { x: number; y: number; z: number; yaw: number; pitch: number; bank: number; speed: number; steer: number; throttle: number; brake: number; rotor: number; thrust: number; fuel: number; landed: boolean; phase?: FlightPhase };
/** Physical aircraft phases, in the engine's `FlightPhase` order (the trace's optional 15th column). */
export const FLIGHT_PHASES = ["Parked", "Taxi", "Take-off roll", "Rotate", "Climb", "Cruise", "Pattern entry", "Downwind", "Base", "Final", "Flare", "Rollout", "Go-around", "Manual"] as const;
export type FlightPhase = (typeof FLIGHT_PHASES)[number];
export type LocomotionTrace = { type: string; frameSeconds: number; course: { x: number; y: number; z: number }[]; frames: LocomotionFrame[] };

/** Parses `--simulate-locomotion` output: header, course points, then 14 values per frame. */
export function parseLocomotionTrace(text: string): LocomotionTrace {
  const [header, courseLine, ...lines] = text.trim().split("\n");
  const match = header?.match(/^locomotion-preview 1 type=(\w+) frames=(\d+) frame_seconds=([\d.]+)/);
  if (!match || !courseLine?.startsWith("course")) throw new Error("Unexpected locomotion preview output.");
  const values = courseLine.slice("course".length).trim().split(/\s+/).filter(Boolean).map(Number);
  if (values.length % 3 || values.some(n => !Number.isFinite(n))) throw new Error("Locomotion preview course is malformed.");
  const course = Array.from({ length: values.length / 3 }, (_, i) => ({ x: values[i * 3], y: values[i * 3 + 1], z: values[i * 3 + 2] }));
  const frames = lines.map(line => {
    const n = line.trim().split(/\s+/).map(Number);
    if ((n.length !== 14 && n.length !== 15) || n.some(v => !Number.isFinite(v)) || (n.length === 15 && !FLIGHT_PHASES[n[14]])) throw new Error("Locomotion preview frames are malformed.");
    return { x: n[0], y: n[1], z: n[2], yaw: n[3], pitch: n[4], bank: n[5], speed: n[6], steer: n[7], throttle: n[8], brake: n[9], rotor: n[10], thrust: n[11], fuel: n[12], landed: n[13] === 1, ...(n.length === 15 ? { phase: FLIGHT_PHASES[n[14]] } : {}) };
  });
  if (frames.length !== Number(match[2])) throw new Error("Locomotion preview is incomplete.");
  return { type: match[1], frameSeconds: Number(match[3]), course, frames };
}
