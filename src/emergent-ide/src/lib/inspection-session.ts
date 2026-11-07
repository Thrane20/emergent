import "server-only";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { projectTargetDir } from "./workspace";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { readdir, stat } from "node:fs/promises";
import { InspectionDecoder, inspectionCommand, type InspectionFrame, type ToolResult } from "./inspection-protocol";
import type { ProjectSnapshot } from "./project";
import { projectScenes } from "./project-scenes";
import {foundationRoot} from "./project-code-context";
import {raymarchedSettings,type RaymarchedSettings} from "./raymarched-terrain";
import {stringify} from "smol-toml";

type Binding=Pick<ProjectSnapshot,"id"|"root">;
type Session = {
  id: string; projectId: string; root: string; scene: string; state: "building"|"running"|"stopping"|"stopped"|"failed";
  child: ChildProcessWithoutNullStreams; latest: InspectionFrame | null; log: string; error: string | null;
  used: number; seq: number; started: number; waiters: Set<() => void>;
  pending: Map<number,{resolve:(result?:ToolResult)=>void;reject:(error:Error)=>void}>;
  lease: ReturnType<typeof setInterval>; parked: string | null;
};

// A parked worker stays warm while the IDE is on another page; it exits only after this much idle time.
const PARKED_IDLE_MS = 30 * 60_000;

/** Cheap change fingerprint of the files a scene render depends on. */
async function projectFingerprint(root: string): Promise<string> {
  let newest = 0, count = 0;
  const walk = async (dir: string): Promise<void> => {
    for (const entry of await readdir(dir, { withFileTypes: true }).catch(() => [])) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) await walk(full);
      else { const info = await stat(full).catch(() => null); if (info) { count++; newest = Math.max(newest, info.mtimeMs); } }
    }
  };
  await Promise.all(["assets", "src", "emergent.toml"].map(async name => {
    const full = path.join(root, name), info = await stat(full).catch(() => null);
    if (info?.isDirectory()) await walk(full); else if (info) { count++; newest = Math.max(newest, info.mtimeMs); }
  }));
  return `${count}:${newest}`;
}
export type InspectionStatus = {id:string;scene:string;state:string;error:string|null;diagnostics:string;sequence:number;workerPid:number|null;tick:number;renderUs:number;encodeUs:number;uptimeMs:number};

export class InspectionSessions {
  private active: Session | null = null;
  private starting = false;
  async startTerrain(project:ProjectSnapshot,value:unknown){
    if(this.starting)throw new Error("An inspection session is already starting.");this.starting=true;
    try{const settings=raymarchedSettings(value),scene="@terrain:"+randomUUID();return await this.launch(project,scene,settings);}finally{this.starting=false;}
  }
  async start(project: ProjectSnapshot, scene: string) {
    if(this.starting)throw new Error("An inspection session is already starting.");
    this.starting=true;
    try{return await this.launch(project,scene);}finally{this.starting=false;}
  }
  status(project: Binding, id: string): InspectionStatus {
    const s=this.get(project,id);
    return {id:s.id,scene:s.scene,state:s.state,error:s.error,diagnostics:s.log,sequence:s.latest?.sequence??0,workerPid:s.latest?.workerPid??null,tick:0,renderUs:s.latest?.renderUs??0,encodeUs:s.latest?.encodeUs??0,uptimeMs:Date.now()-s.started};
  }
  private async launch(project: ProjectSnapshot, scene: string, terrain?:RaymarchedSettings) {
    if (!terrain&&!projectScenes(project).some(item=>item.path===scene)) throw new Error("Choose a scene from the linked project.");
    if (this.active && ["building","running"].includes(this.active.state) && this.active.root===project.root && this.active.projectId===project.id && this.active.scene===scene) {
      const parked=this.active.parked;this.active.parked=null;this.active.used=Date.now();
      if(!parked||parked===await projectFingerprint(project.root))return this.status(project,this.active.id);
    }
    if (this.active) await this.terminate(this.active);
    const root=terrain?await foundationRoot(project):project.root;
    const args=terrain?["run","--release","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--example","terrain_live_preview","--features","graphics","--",stringify(terrain)]:["run","--release","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--target-dir",projectTargetDir(root),"--","--inspect-scene",scene];
    const child=spawn("cargo",args,{cwd:root,shell:false,detached:process.platform!=="win32",stdio:["pipe","pipe","pipe"]});
    const s: Session={id:randomUUID(),projectId:project.id,root:project.root,scene,state:"building",child,latest:null,log:"",error:null,used:Date.now(),started:Date.now(),seq:0,waiters:new Set(),pending:new Map(),parked:null,lease:setInterval(()=>{
      if(Date.now()-s.used>PARKED_IDLE_MS) void this.terminate(s);
      else if(s.state==="building" && Date.now()-s.started>120000)this.fail(s,"Inspection startup exceeded 120 seconds.");
    },5000)};
    s.lease.unref(); this.active=s;
    const decoder=new InspectionDecoder(frame=>{
      if(this.active!==s || !["building","running"].includes(s.state))return;
      s.latest=frame;s.state="running";for(const notify of [...s.waiters])notify();
    },(seq,error,result)=>{const pending=s.pending.get(seq);if(pending){s.pending.delete(seq);if(error)pending.reject(new Error(error));else pending.resolve(result);}});
    child.stdout.on("data",chunk=>{try{decoder.push(chunk);}catch(error){this.fail(s,error instanceof Error?error.message:"Invalid worker output.");}});
    child.stderr.on("data",chunk=>{s.log=(s.log+chunk.toString()).slice(-16000);});
    child.stdin.on("error",error=>{if(!["stopped","stopping"].includes(s.state))this.fail(s,error.message);});
    child.on("error",error=>this.fail(s,error.message));
    child.on("close",()=>{
      clearInterval(s.lease);
      if(s.state==="stopping")s.state="stopped";
      else if(s.state!=="failed" && s.state!=="stopped"){s.state="failed";s.error=s.log.trim()||"Inspection process exited.";}
      this.rejectPending(s);for(const notify of [...s.waiters])notify();
    });
    return this.status(project,s.id);
  }
  async command(project:Binding,id:string,value:unknown) {
    const s=this.get(project,id);
    if(s.state!=="running")throw new Error(s.error||"Inspection is not running.");
    if(value&&typeof value==="object"&&"op" in value&&value.op==="preview"&&!s.scene.startsWith("@terrain:"))throw new Error("Preview options require a terrain authoring session.");
    if(s.pending.size>=8 || s.child.stdin.writableLength>8192)throw new Error("Inspection command queue is full.");
    const seq=++s.seq;const text=inspectionCommand(value,seq);
    const result=await new Promise<ToolResult|undefined>((resolve,reject)=>{
      const timer=setTimeout(()=>{s.pending.delete(seq);reject(new Error("Inspection command acknowledgement timed out."));},10000);
      s.pending.set(seq,{resolve:result=>{clearTimeout(timer);resolve(result);},reject:error=>{clearTimeout(timer);reject(error);}});
      s.child.stdin.write(text);
    });
    return {...this.status(project,id),commandSequence:seq,result};
  }
  async frame(project:Binding,id:string,after:number,signal:AbortSignal):Promise<InspectionFrame|null> {
    const s=this.get(project,id);
    if(s.latest && s.latest.sequence>after)return s.latest;
    if(!["building","running"].includes(s.state))throw new Error(s.error||"Inspection stopped.");
    if(s.waiters.size>=4)throw new Error("Too many inspection frame subscribers.");
    await new Promise<void>(resolve=>{
      const done=()=>{clearTimeout(timer);signal.removeEventListener("abort",done);s.waiters.delete(done);resolve();};
      const timer=setTimeout(done,10000);s.waiters.add(done);signal.addEventListener("abort",done,{once:true});if(signal.aborted)done();
    });
    if(s.state==="failed")throw new Error(s.error||"Inspection failed.");
    return s.latest && s.latest.sequence>after?s.latest:null;
  }
  /** The running session showing `scene` for this project, if any; used to push external edits live. */
  activeFor(project:Binding,scene:string):string|null {
    const s=this.active;
    return s&&s.state==="running"&&s.projectId===project.id&&s.root===project.root&&s.scene===scene?s.id:null;
  }
  /** Ends whichever session is active, for any project; used when the IDE switches projects. */
  /** Marks the session as left running by a page that navigated away, so a return can tell whether it went stale. */
  async park(project:Binding,id:string){const s=this.get(project,id);s.parked=await projectFingerprint(project.root);}
  async stopActive() {if(this.active)await this.terminate(this.active);}
  async stop(project:Binding,id:string) {const s=this.get(project,id);await this.terminate(s);return this.status(project,id);}
  private get(project:Binding,id:string) {
    const s=this.active;
    if(!s || s.id!==id || s.projectId!==project.id || s.root!==project.root)throw new Error("Inspection session is stale or belongs to another project.");
    s.used=Date.now();return s;
  }
  private kill(s:Session,signal:NodeJS.Signals="SIGTERM") {if(s.child.exitCode!==null || s.child.signalCode!==null)return;try{if(s.child.pid && process.platform!=="win32")process.kill(-s.child.pid,signal);else s.child.kill(signal);}catch{/* The owned process may already have exited. */}}
  private rejectPending(s:Session) {for(const item of s.pending.values())item.reject(new Error(s.error||"Inspection stopped."));s.pending.clear();}
  private fail(s:Session,message:string) {if(["stopped","stopping","failed"].includes(s.state))return;s.error=message;s.state="failed";clearInterval(s.lease);this.kill(s);setTimeout(()=>this.kill(s,"SIGKILL"),2000).unref();this.rejectPending(s);for(const notify of [...s.waiters])notify();}
  private async terminate(s:Session) {
    clearInterval(s.lease);if(["stopped","failed"].includes(s.state)){this.kill(s);return;}
    s.state="stopping";this.rejectPending(s);
    await new Promise<void>(resolve=>{
      const done=()=>{clearTimeout(timer);s.child.removeListener("close",done);s.state="stopped";for(const notify of [...s.waiters])notify();resolve();};
      const timer=setTimeout(()=>{this.kill(s,"SIGKILL");done();},2000);s.child.once("close",done);
      if(!s.child.stdin.destroyed)s.child.stdin.end(`request={op="stop",seq=${++s.seq}}\n`);else this.kill(s);
    });
  }
}
const state=globalThis as typeof globalThis & {emergentInspectionSessions?:InspectionSessions};
export const inspectionSessions=state.emergentInspectionSessions??=new InspectionSessions();
// Preserve owned processes across dev reloads while updating manager methods.
Object.setPrototypeOf(inspectionSessions,InspectionSessions.prototype);
