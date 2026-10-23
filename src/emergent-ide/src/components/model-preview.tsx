"use client";

import { useEffect, useRef, useState, type HTMLAttributes, type Ref } from "react";

type Viewer = HTMLElement & { cameraOrbit: string; cameraTarget: string; fieldOfView: string; jumpCameraToGoal: () => void };
declare module "react" {
  // eslint-disable-next-line @typescript-eslint/no-namespace -- React's JSX custom-element augmentation uses this namespace.
  namespace JSX {
    interface IntrinsicElements {
      "model-viewer": HTMLAttributes<Viewer> & {
        ref?: Ref<Viewer>; src: string; alt: string; "camera-controls"?: boolean;
        "touch-action"?: string; "camera-orbit"?: string; "shadow-intensity"?: string;
        exposure?: string; loading?: string;
      };
    }
  }
}

export function ModelPreview({ src, name }: { src: string; name: string }) {
  const ref = useRef<Viewer>(null);
  const [registered, setRegistered] = useState(false);
  const [state, setState] = useState("Loading model viewer…");
  useEffect(() => {
    let active = true;
    void import("@google/model-viewer").then(() => { if (active) setRegistered(true); }).catch(() => { if (active) setState("Unable to load the model viewer."); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    const viewer = ref.current;
    if (!viewer) return;
    const load = () => setState("");
    const error = () => setState("This model could not be displayed. Its format or materials may need conversion.");
    viewer.addEventListener("load", load);
    viewer.addEventListener("error", error);
    return () => { viewer.removeEventListener("load", load); viewer.removeEventListener("error", error); };
  }, [registered]);
  function reset() {
    if (!ref.current) return;
    ref.current.cameraOrbit = "0deg 75deg 105%";
    ref.current.cameraTarget = "auto auto auto";
    ref.current.fieldOfView = "auto";
    ref.current.jumpCameraToGoal();
  }
  return <div className="model-preview">
    {registered && <model-viewer ref={ref} src={src} alt={name} camera-controls touch-action="pan-y" camera-orbit="0deg 75deg 105%" shadow-intensity="0.7" exposure="1" loading="eager" />}
    {state && <p className="model-viewer-status" role="status">{state}</p>}
    <div className="model-preview-tools"><span>Drag to orbit · Scroll to zoom</span><button className="quiet-button" onClick={reset} disabled={!registered}>Reset view</button></div>
  </div>;
}
