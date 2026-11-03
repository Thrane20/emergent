import {readProjectForPage} from "@/lib/project-page";
import {shaderLibrary} from "@/lib/shader-assets";
import {ShaderEditor} from "@/components/shader-editor";
export const instant=false;
export default async function ShadersPage(){const project=await readProjectForPage();return <ShaderEditor projectId={project.id} projectName={project.name} initialItems={await shaderLibrary(project)}/>;}
