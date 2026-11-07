import {falBalance} from "@/lib/fal-balance";
export async function GET(request:Request){
 try{const params=new URL(request.url).searchParams;return Response.json(await falBalance.get(params.get("refresh")==="1",params.get("cached")==="1"),{headers:{"Cache-Control":"private, no-store"}});}
 catch{return Response.json({state:"unavailable",balance:null,currency:"USD",message:"Unable to read fal settings.",checkedAt:null},{headers:{"Cache-Control":"private, no-store"}});}
}
