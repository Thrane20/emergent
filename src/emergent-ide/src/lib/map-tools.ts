// Scene map tools. Each declares a gesture; creating tools turn the finished gesture into
// map items. Adding a tool is one entry here (and a gesture case if it needs a new one).
import type { MapItemInput } from "./scene-map-types";

export type MapGesture = "select" | "point" | "polyline" | "link" | "circle" | "rect";
export type GestureResult =
  | { gesture: "point"; x: number; z: number }
  | { gesture: "polyline"; points: [number, number][] }
  | { gesture: "circle"; x: number; z: number; radius: number }
  | { gesture: "rect"; x0: number; z0: number; x1: number; z1: number };
export type MapTool = {
  id: string;
  label: string;
  /** Shown while the tool is armed. */
  hint: string;
  /** SVG path data on a 24 × 24 grid. */
  icon: string;
  gesture: MapGesture;
  /** Palette section; tools without one are the general map tools. */
  group?: string;
  /** Items to add when the gesture finishes; `path` links them in order under a new group. */
  build?: (result: GestureResult) => { items: MapItemInput[]; path?: boolean; name?: string };
};

/** Smallest drag that counts as drawing a size; anything shorter places the default size. */
export const MIN_DRAG_METRES = 0.5;

export const MAP_TOOLS: MapTool[] = [
  { id: "select", label: "Select", hint: "Click to select, drag to move, drag empty space to pan. Delete removes the selection.", icon: "M5 3l13 8-6 1.5L9 19z", gesture: "select" },
  {
    id: "waypoint", label: "Waypoint", hint: "Click the map to drop a waypoint.", icon: "M12 21s-6-6.5-6-11a6 6 0 1 1 12 0c0 4.5-6 11-6 11zm0-9a2 2 0 1 0 0-4 2 2 0 0 0 0 4z", gesture: "point",
    build: result => result.gesture === "point" ? { items: [{ kind: "waypoint", x: result.x, z: result.z }] } : { items: [] },
  },
  {
    id: "path", label: "Waypoint path", hint: "Click to add waypoints in order; Enter or double-click finishes, Esc cancels. They're linked and grouped.", icon: "M4 19l5-7 5 3 6-9M4 19h.01M9 12h.01M14 15h.01M20 6h.01", gesture: "polyline",
    build: result => result.gesture === "polyline" ? { items: result.points.map(([x, z]) => ({ kind: "waypoint" as const, x, z })), path: result.points.length > 1 } : { items: [] },
  },
  { id: "link", label: "Link", hint: "Click a waypoint, then another, to add or remove the link between them.", icon: "M9 15l6-6M8 11l-2 2a3 3 0 0 0 4 4l2-2m2-6 2-2a3 3 0 0 0-4-4l-2 2", gesture: "link" },
  {
    id: "circle", label: "Circle zone", hint: "Press at the centre and drag out the radius (a click makes 5 m).", icon: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zm0-8.5h.01", gesture: "circle",
    build: result => result.gesture === "circle" ? { items: [{ kind: "zone", shape: "circle", x: result.x, z: result.z, radius: result.radius >= MIN_DRAG_METRES ? result.radius : 5 }] } : { items: [] },
  },
  {
    id: "rect", label: "Rectangle zone", hint: "Drag from corner to corner (a click makes 10 × 10 m). Turn it later with the object's rotation.", icon: "M4 5h16v14H4z", gesture: "rect",
    build: result => {
      if (result.gesture !== "rect") return { items: [] };
      const width = Math.abs(result.x1 - result.x0), depth = Math.abs(result.z1 - result.z0);
      const sized = width >= MIN_DRAG_METRES && depth >= MIN_DRAG_METRES;
      return { items: [{ kind: "zone", shape: "rect", x: sized ? (result.x0 + result.x1) / 2 : result.x0, z: sized ? (result.z0 + result.z1) / 2 : result.z0, width: sized ? width : 10, depth: sized ? depth : 10 }] };
    },
  },
  // Airport items: the same map items with `aeroway` tags (OpenStreetMap values) that the
  // engine's tower reads. See docs/design/airport-operations.md.
  ...airportTools(),
];

function airportTools(): MapTool[] {
  const point = (id: string, label: string, aeroway: string, hint: string, icon: string): MapTool => ({
    id, label, hint, icon, gesture: "point", group: "Airport",
    build: result => result.gesture === "point" ? { items: [{ kind: "waypoint", x: result.x, z: result.z, name: label, metadata: { aeroway } }] } : { items: [] },
  });
  return [
    {
      id: "aerodrome", label: "Aerodrome", group: "Airport", gesture: "circle", icon: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM8 12h8M12 8v8",
      hint: "Drag out the airport boundary. Runways, taxiways and stands inside it belong to this airport.",
      build: result => result.gesture === "circle" ? { items: [{ kind: "zone", shape: "circle", x: result.x, z: result.z, radius: result.radius >= MIN_DRAG_METRES ? result.radius : 1500, name: "Aerodrome", metadata: { aeroway: "aerodrome", pattern_height: 300 } }] } : { items: [] },
    },
    {
      id: "runway", label: "Runway", group: "Airport", gesture: "rect", icon: "M10 3h4v18h-4zM12 6v2M12 11v2M12 16v2",
      hint: "Drag the runway's length and width. Aircraft land along its long side (tags: direction, circuit=left|right).",
      build: result => {
        if (result.gesture !== "rect") return { items: [] };
        const dx = Math.abs(result.x1 - result.x0), dz = Math.abs(result.z1 - result.z0);
        const sized = Math.max(dx, dz) >= MIN_DRAG_METRES;
        // The long side becomes the zone's depth (local Z), the landing axis.
        const length = sized ? Math.max(dx, dz) : 900, width = sized ? Math.max(Math.min(dx, dz), 10) : 30;
        return { items: [{ kind: "zone", shape: "rect", x: sized ? (result.x0 + result.x1) / 2 : result.x0, z: sized ? (result.z0 + result.z1) / 2 : result.z0, width, depth: length, yaw: dx > dz ? 90 : 0, name: "Runway", metadata: { aeroway: "runway", direction: "both", circuit: "left" } }] };
      },
    },
    {
      id: "taxiway", label: "Taxiway", group: "Airport", gesture: "polyline", icon: "M4 20c4-8 12-8 16-16M4 20h.01M12 12h.01M20 4h.01",
      hint: "Click along the taxiway; Enter or double-click finishes. Points are linked both ways and grouped.",
      build: result => result.gesture === "polyline" ? { items: result.points.map(([x, z]) => ({ kind: "waypoint" as const, x, z, metadata: { aeroway: "taxiway" } })), path: result.points.length > 1, name: "Taxiway" } : { items: [] },
    },
    point("holding_position", "Holding position", "holding_position", "Click beside a runway end, then Link it to the taxiway. Departures wait here.", "M5 12h14M5 8h14M5 16h14"),
    point("stand", "Stand", "parking_position", "Click a parking spot, then Link it to a taxiway point.", "M6 4h7a4 4 0 0 1 0 8H9v8H6z"),
    point("tower", "Tower", "tower", "Click to place the control tower: the airport sequences its traffic.", "M10 21V9h4v12M8 9h8l-1-5H9z"),
  ];
}
