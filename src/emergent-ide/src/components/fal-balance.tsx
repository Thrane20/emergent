"use client";
import {useCallback,useEffect,useRef,useState} from "react";
import type {FalBalance as Balance} from "@/lib/fal-balance-types";
export function FalBalance(){
 const [balance,setBalance]=useState<Balance|null>(null),pending=useRef<AbortController|null>(null);
 const refresh=useCallback(async(force=false)=>{
  pending.current?.abort();const controller=new AbortController();pending.current=controller;
  try{const response=await fetch(`/api/fal-balance${force?"?refresh=1":""}`,{cache:"no-store",signal:controller.signal});if(!response.ok)throw new Error("Balance unavailable");const data:Balance=await response.json();if(!controller.signal.aborted)setBalance(data);}
  catch{if(!controller.signal.aborted)setBalance({state:"unavailable",balance:null,currency:"USD",message:"Unable to check fal balance. Click to retry.",checkedAt:null});}
 },[]);
 useEffect(()=>{
  const check=()=>{if(document.visibilityState==="visible")void refresh();},changed=()=>void refresh(true),initial=setTimeout(check,0),timer=setInterval(check,60000);
  window.addEventListener("focus",check);window.addEventListener("fal-settings-changed",changed);document.addEventListener("visibilitychange",check);
  return()=>{pending.current?.abort();clearTimeout(initial);clearInterval(timer);window.removeEventListener("focus",check);window.removeEventListener("fal-settings-changed",changed);document.removeEventListener("visibilitychange",check);};
 },[refresh]);
 const amount=balance?.state==="no-key"?"$0":balance?.state==="ready"?new Intl.NumberFormat("en-US",{style:"currency",currency:balance.currency,currencyDisplay:"narrowSymbol"}).format(balance.balance!):balance?"Unavailable":"…";
 return <button type="button" className="fal-balance" aria-label="fal balance" aria-busy={balance===null} title={balance?.message??"Checking fal account balance…"} onClick={()=>void refresh(true)}><span>fal</span><strong aria-live="polite">{amount}</strong>{balance?.state==="ready"&&<small>{balance.currency}</small>}</button>;
}
