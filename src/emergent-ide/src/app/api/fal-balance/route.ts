import {falBalance} from "@/lib/fal-balance";
export async function GET(request:Request){
 try{return Response.json(await falBalance.get(new URL(request.url).searchParams.get("refresh")==="1"),{headers:{"Cache-Control":"private, no-store"}});}
 catch{return Response.json({state:"unavailable",balance:null,currency:"USD",message:"Unable to read fal settings.",checkedAt:null},{headers:{"Cache-Control":"private, no-store"}});}
}
