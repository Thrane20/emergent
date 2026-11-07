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
  // A box marking where an airport function happens; a click places the default size.
  const box = (id: string, label: string, aeroway: string, size: [number, number], hint: string, icon: string): MapTool => ({
    id, label, hint, icon, gesture: "rect", group: "Airport",
    build: result => {
      if (result.gesture !== "rect") return { items: [] };
      const width = Math.abs(result.x1 - result.x0), depth = Math.abs(result.z1 - result.z0);
      const sized = width >= MIN_DRAG_METRES && depth >= MIN_DRAG_METRES;
      return { items: [{ kind: "zone", shape: "rect", x: sized ? (result.x0 + result.x1) / 2 : result.x0, z: sized ? (result.z0 + result.z1) / 2 : result.z0, width: sized ? width : size[0], depth: sized ? depth : size[1], name: label, metadata: { aeroway } }] };
    },
  });
  return [
    {
      id: "aerodrome", label: "Aerodrome", group: "Airport", gesture: "circle", icon: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM8 12h8M12 8v8",
      hint: "Drag out the airport boundary. Runways, taxiways and stands inside it belong to this airport. Set its scale tag below 1 for a miniature airport (0.1: circuits, heights and spacing a tenth of full size).",
      build: result => result.gesture === "circle" ? { items: [{ kind: "zone", shape: "circle", x: result.x, z: result.z, radius: result.radius >= MIN_DRAG_METRES ? result.radius : 1500, name: "Aerodrome", metadata: { aeroway: "aerodrome", pattern_height: 300, scale: 1 } }] } : { items: [] },
    },
    {
      id: "runway", label: "Runway", group: "Airport", gesture: "rect", icon: "M10 3h4v18h-4zM12 6v2M12 11v2M12 16v2",
      hint: "Drag the runway's length and width. Aircraft land along its long side (tags: direction, circuit=left|right).",
      build: result => {
        if (result.gesture !== "rect") return { items: [] };
        const dx = Math.abs(result.x1 - result.x0), dz = Math.abs(result.z1 - result.z0);
        const sized = Math.max(dx, dz) >= MIN_DRAG_METRES;
        // The long side becomes the zone's depth (local Z), the landing axis.
        const length = sized ? Math.max(dx, dz) : 900, width = sized ? Math.max(Math.min(dx, dz), 1) : 30;
        return { items: [{ kind: "zone", shape: "rect", x: sized ? (result.x0 + result.x1) / 2 : result.x0, z: sized ? (result.z0 + result.z1) / 2 : result.z0, width, depth: length, yaw: dx > dz ? 90 : 0, name: "Runway", metadata: { aeroway: "runway", direction: "both", circuit: "left" } }] };
      },
    },
    box("threshold", "Threshold", "threshold", [30, 60], "Drag a box over a runway end: landings aim from it and take-offs line up in it. Only ends with a threshold are used (none: both).", "M6 3h12v6H6zM12 9v12M9 13h6M9 17h6"),
    {
      id: "taxiway", label: "Taxiway", group: "Airport", gesture: "polyline", icon: "M4 20c4-8 12-8 16-16M4 20h.01M12 12h.01M20 4h.01",
      hint: "Click along a taxiway centreline; Enter or double-click finishes it, then draw the next. Lines join where they cross or where an end comes within 15 m of another line.",
      build: result => result.gesture === "polyline" ? { items: result.points.map(([x, z]) => ({ kind: "waypoint" as const, x, z, metadata: { aeroway: "taxiway" } })), path: result.points.length > 1, name: "Taxiway" } : { items: [] },
    },
    {
      id: "pattern", label: "Traffic pattern", group: "Airport", gesture: "polyline", icon: "M5 19V7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v8M19 15l-3-3m3 3 3-3M5 19h.01",
      hint: "Click the pattern in flying order, ending on final before a threshold box; Enter finishes. Labelled entry, downwind…, base, final: set each point's entry/exit courses and altitude in the inspector.",
      build: result => {
        if (result.gesture !== "polyline") return { items: [] };
        const count = result.points.length;
        // Default legs: the first point joins, the last two are base and final, the rest downwind.
        const leg = (i: number) => count > 1 && i === count - 1 ? "final" : count > 2 && i === count - 2 ? "base" : i === 0 && count > 3 ? "entry" : "downwind";
        return { items: result.points.map(([x, z], i) => ({ kind: "waypoint" as const, x, z, name: `Pattern ${leg(i)}`, metadata: { aeroway: "circuit", leg: leg(i) } })), path: count > 1, name: "Traffic pattern" };
      },
    },
    point("holding_position", "Holding position", "holding_position", "Click beside a runway end. Departures wait here; it joins the nearest taxiway line.", "M5 12h14M5 8h14M5 16h14"),
    box("stand", "Stand", "parking_position", [30, 30], "Drag a box over a parking spot. Aircraft park at its centre; it joins the nearest taxiway line, so draw a taxilane in front of the stands.", "M6 4h7a4 4 0 0 1 0 8H9v8H6z"),
    box("tower", "Tower", "tower", [10, 10], "Drag a box where the control tower stands: the airport sequences its traffic.", "M10 21V9h4v12M8 9h8l-1-5H9z"),
  ];
}
