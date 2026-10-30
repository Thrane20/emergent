// Authored object components: the registry exported by the game binary and per-instance [[entities.components]] lists.
// The Rust loader is the authority on field validity; these helpers only check shape so edits can be built safely.
export type ComponentField = { name: string; label: string; kind: "number" | "colour" | "bool" | "vector3" | "entity" | "entities" | "text" | "choice" | "brain"; options?: string[]; min?: number; max?: number; unit?: string; variants?: string[]; default?: ComponentValue; help: string };
export type ComponentType = { id: string; label: string; description: string; applies_to: string; fields: ComponentField[] };
export type AuthoredComponent = { type: string; enabled: boolean; fields: Record<string, ComponentValue> };
export type ComponentValue = number | boolean | number[] | string | string[];
/** Field changes for one component: a value sets it, null resets it to the asset default; `enabled` toggles the component. */
export type ComponentPatch = Record<string, ComponentValue | null>;

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);
const fieldValue = (value: unknown): value is ComponentValue => typeof value === "boolean" || typeof value === "string" && value.length > 0 && value.length <= 512 || Array.isArray(value) && value.length <= 64 && value.every(item => typeof item === "string" && item.length > 0 && item.length <= 128) || typeof value === "number" && Number.isFinite(value) || Array.isArray(value) && value.length <= 4 && value.every(n => typeof n === "number" && Number.isFinite(n));
const typeId = (value: unknown): value is string => typeof value === "string" && /^[a-z][a-z0-9_]*\.[a-z][a-z0-9_.]*$/.test(value) && value.length <= 96;

export function componentTypes(exported: unknown): ComponentType[] {
  if (!object(exported) || !Array.isArray(exported.types) || exported.types.length > 256) throw new Error("Invalid component registry export.");
  return exported.types.map(value => {
    if (!object(value) || !typeId(value.id) || typeof value.label !== "string" || typeof value.applies_to !== "string" || !Array.isArray(value.fields)) throw new Error("Invalid component type in registry export.");
    const fields = value.fields.map(field => {
      if (!object(field) || typeof field.name !== "string" || typeof field.label !== "string" || !["number", "colour", "bool", "vector3", "entity", "entities", "text", "choice", "brain"].includes(field.kind as string)) throw new Error(`Invalid field in component ${value.id}.`);
      return { ...field, default: fieldValue(field.default) ? field.default : undefined, variants: Array.isArray(field.variants) ? field.variants.filter((v): v is string => typeof v === "string") : undefined, help: typeof field.help === "string" ? field.help : "" } as ComponentField;
    });
    return { id: value.id, label: value.label, description: typeof value.description === "string" ? value.description : "", applies_to: value.applies_to, fields };
  });
}

export function authoredComponents(value: unknown): AuthoredComponent[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 64) throw new Error("Instance components must be a list of [[entities.components]] tables.");
  return value.map(entry => {
    if (!object(entry) || !typeId(entry.type) || entry.enabled !== undefined && typeof entry.enabled !== "boolean") throw new Error("Each component needs a type like emergent.light.");
    const { type, enabled, ...fields } = entry;
    if (!Object.values(fields).every(fieldValue)) throw new Error(`Component ${type} has a value that is not a number, colour or true/false.`);
    return { type, enabled: enabled !== false, fields: fields as Record<string, ComponentValue> };
  });
}

/**
 * Apply a patch to one component. For intrinsic types (a light's Light), an enabled component with no fields left is
 * removed so the asset defaults apply; added types (`keepEmpty`) stay until removed with a null patch.
 */
export function patchComponents(current: AuthoredComponent[], type: string, patch: ComponentPatch | null, keepEmpty = false): AuthoredComponent[] {
  if (!typeId(type)) throw new Error("Choose a component type like emergent.light.");
  const rest = current.filter(component => component.type !== type);
  if (patch === null) return rest;
  if (!object(patch)) throw new Error("Component changes must be an object of field values.");
  const existing = current.find(component => component.type === type), next: AuthoredComponent = { type, enabled: existing?.enabled ?? true, fields: { ...existing?.fields } };
  for (const [name, value] of Object.entries(patch)) {
    if (name === "enabled") { if (typeof value !== "boolean") throw new Error("enabled must be true or false."); next.enabled = value; continue; }
    if (name === "type" || !/^[a-z][a-z0-9_]{0,63}$/.test(name)) throw new Error(`Invalid component field '${name}'.`);
    if (value === null) delete next.fields[name];
    else if (fieldValue(value)) next.fields[name] = value;
    else throw new Error(`${type}.${name} must be a number, vector, colour, true/false or an instance ID.`);
  }
  const index = current.findIndex(component => component.type === type);
  if (!keepEmpty && next.enabled && !Object.keys(next.fields).length) return rest;
  return index < 0 ? [...rest, next] : [...current.slice(0, index), next, ...current.slice(index + 1)];
}

export function componentTables(list: AuthoredComponent[]) {
  return list.map(component => ({ type: component.type, ...(component.enabled ? {} : { enabled: false }), ...component.fields }));
}

/** Types shown on an object: those matching its kind (e.g. a light) plus any already authored on it. */
export function objectComponentTypes(types: ComponentType[], kind: string | undefined, authored: AuthoredComponent[]) {
  return types.filter(type => type.applies_to === kind || authored.some(component => component.type === type.id));
}
