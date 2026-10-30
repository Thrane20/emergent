import {parse} from "smol-toml";
export type NoiseSettings={algorithm_version:number;algorithm:"perlin"|"open-simplex"|"worley";seed:number;dimensions:number;size:number;frequency:number;octaves:number;lacunarity:number;persistence:number;extent:number;offset:number[]};
export const DEFAULT_NOISE:NoiseSettings={algorithm_version:1,algorithm:"perlin",seed:1,dimensions:2,size:128,frequency:1,octaves:4,lacunarity:2,persistence:0.5,extent:8,offset:[0,0,0]};
export function noiseSettings(value:unknown):NoiseSettings {
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Noise settings are required.");
 const input=value as Record<string,unknown>;if(Object.keys(input).some(key=>!Object.hasOwn(DEFAULT_NOISE,key)))throw new Error("Unknown noise setting.");
 const s={...DEFAULT_NOISE,...input} as NoiseSettings;
 if(s.algorithm_version!==1||!["perlin","open-simplex","worley"].includes(s.algorithm))throw new Error("Unsupported noise algorithm/version.");
 for(const [key,min,max]of[["seed",0,4294967295],["dimensions",2,3],["size",2,s.dimensions===2?512:128],["octaves",1,8]]as const)if(!Number.isInteger(s[key])||s[key]<min||s[key]>max)throw new Error(`${key} must be an integer in ${min}..${max}.`);
 for(const [key,min,max]of[["frequency",0.0001,64],["lacunarity",1,4],["persistence",0,1],["extent",0.001,1024]]as const)if(!Number.isFinite(s[key])||s[key]<min||s[key]>max)throw new Error(`${key} must be finite and in ${min}..${max}.`);
 if(!Array.isArray(s.offset)||s.offset.length!==3||s.offset.some(v=>typeof v!=="number"||!Number.isFinite(v)||Math.abs(v)>10000))throw new Error("Offset needs three finite axes in -10000..10000.");
 if(s.offset.some(v=>(Math.abs(v)+s.extent)*s.frequency*s.lacunarity**(s.octaves-1)>1e8))throw new Error("Noise coordinates exceed the supported range.");
 if(s.size**s.dimensions*s.octaves>2097152)throw new Error("Field exceeds 2,097,152 octave evaluations. Reduce resolution or octaves.");
 return s;
}
export function noiseSource(source:string){const document=parse(source);if(document.schema_version!==1||document.kind!=="noise-function")throw new Error("Choose a schema-1 noise-function asset.");return {document,settings:noiseSettings(document.noise)};}
export function replaceNoiseSettings(source:string,value:unknown){
 const settings=noiseSettings(value),{document}=noiseSource(source);
 const header=/^\[noise\][ \t]*(?:#.*)?\r?$/m.exec(source);if(!header)throw new Error("Noise form editing requires a [noise] table. Use the TOML tab to convert this source.");
 const comments=source.slice(header.index).split("\n").map(line=>line.match(/#.*$/)?.[0]).filter(Boolean).join("\n");
 const content=source.slice(0,header.index)+`[noise]\n${comments?comments+"\n":""}`+Object.entries(settings).map(([key,v])=>`${key} = ${JSON.stringify(v)}`).join("\n")+"\n";
 document.noise=settings;
 // Preserve everything outside the settings table, including authored metadata.
 if(JSON.stringify(parse(content))!==JSON.stringify(document))throw new Error("Could not safely edit the noise table; use TOML source.");
 return content;
}
export type NoiseField={width:number;height:number;depth:number;values:Float32Array;min:number;max:number};
export function decodeNoiseField(buffer:ArrayBuffer):NoiseField {
 if(buffer.byteLength<16||buffer.byteLength>16+2097152*4)throw new Error("Invalid noise field length.");
 const view=new DataView(buffer);if(view.getUint32(0,false)!==0x454e4631)throw new Error("Invalid noise field signature.");
 const width=view.getUint32(4,true),height=view.getUint32(8,true),depth=view.getUint32(12,true),count=width*height*depth;
 if(width<2||width!==height||width>512||(depth!==1&&(depth!==width||width>128))||buffer.byteLength!==16+count*4)throw new Error("Invalid noise field dimensions.");
 const values=new Float32Array(count);let min=1,max=0;
 for(let i=0;i<count;i++){const value=view.getFloat32(16+i*4,true);if(!Number.isFinite(value)||value<0||value>1)throw new Error("Noise samples must be finite values in 0..1.");values[i]=value;min=Math.min(min,value);max=Math.max(max,value);}
 return {width,height,depth,values,min,max};
}
