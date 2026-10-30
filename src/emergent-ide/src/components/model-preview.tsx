"use client";

import { useEffect, useRef, useState, type HTMLAttributes, type Ref } from "react";

type Viewer = HTMLElement & { loaded: boolean; cameraOrbit: string; cameraTarget: string; fieldOfView: string; jumpCameraToGoal: () => void };
declare module "react" {
  // eslint-disable-next-line @typescript-eslint/no-namespace -- React's JSX custom-element augmentation uses this namespace.
  namespace JSX {
    interface IntrinsicElements {
      "model-viewer": HTMLAttributes<Viewer> & {
        ref?: Ref<Viewer>; src: string; alt: string; "camera-controls"?: boolean;
        "touch-action"?: string; "camera-orbit"?: string; "camera-target"?:string; "max-camera-orbit"?:string; "shadow-intensity"?: string;
        exposure?: string; loading?: string; "environment-image"?:string;
      };
    }
  }
}

export function ModelPreview({ src, name, initialOrbit = "0deg 75deg 105%", exposure = 1, environmentImage, initialTarget = "auto auto auto", maximumOrbit, onLoad }: { src: string; name: string; initialOrbit?: string; exposure?:number; environmentImage?:string; initialTarget?:string; maximumOrbit?:string; onLoad?:(src:string)=>void }) {
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
    const load = () => {setState("");onLoad?.(viewer.getAttribute("src")??"");};
    const error = () => setState("This model could not be displayed. Its format or materials may need conversion.");
    viewer.addEventListener("load", load);
    viewer.addEventListener("error", error);
    if (viewer.loaded) load();
    return () => { viewer.removeEventListener("load", load); viewer.removeEventListener("error", error); };
  }, [registered,onLoad]);
  function reset() {
    if (!ref.current) return;
    ref.current.cameraOrbit = initialOrbit;
    ref.current.cameraTarget = initialTarget;
    ref.current.fieldOfView = "auto";
    ref.current.jumpCameraToGoal();
  }
  return <div className="model-preview">
    {registered && <model-viewer ref={ref} src={src} alt={name} camera-controls touch-action="pan-y" camera-orbit={initialOrbit} camera-target={initialTarget} max-camera-orbit={maximumOrbit} shadow-intensity="0.7" environment-image={environmentImage} exposure={String(exposure)} loading="eager" />}
    {state && <p className="model-viewer-status" role="status">{state}</p>}
    <div className="model-preview-tools"><span>Drag to orbit · Scroll to zoom</span><button className="quiet-button" onClick={reset} disabled={!registered}>Reset view</button></div>
  </div>;
}
