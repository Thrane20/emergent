"use client";
import {useCallback,useEffect,useRef,useState} from "react";
import type {FalBalance as Balance} from "@/lib/fal-balance-types";
import {AIBalances} from "./ai-balances";
export function FalBalance(){
 const [balance,setBalance]=useState<Balance|null>(null),[busy,setBusy]=useState(false),pending=useRef<AbortController|null>(null);
 // Mount reads only the server cache; fal is contacted only on refresh or a settings change.
 const refresh=useCallback(async(force:boolean)=>{
  pending.current?.abort();const controller=new AbortController();pending.current=controller;setBusy(true);
  try{const response=await fetch(`/api/fal-balance?${force?"refresh=1":"cached=1"}`,{cache:"no-store",signal:controller.signal});if(!response.ok)throw new Error("Balance unavailable");const data:Balance=await response.json();if(!controller.signal.aborted)setBalance(data);}
  catch{if(!controller.signal.aborted)setBalance({state:"unavailable",balance:null,currency:"USD",message:"Unable to check fal balance. Press refresh to retry.",checkedAt:null});}
  finally{if(!controller.signal.aborted)setBusy(false);}
 },[]);
 useEffect(()=>{
  const changed=()=>void refresh(true),initial=setTimeout(()=>void refresh(false),0);
  window.addEventListener("fal-settings-changed",changed);
  return()=>{pending.current?.abort();clearTimeout(initial);window.removeEventListener("fal-settings-changed",changed);};
 },[refresh]);
 const amount=balance?.state==="no-key"?"$0":balance?.state==="ready"?new Intl.NumberFormat("en-US",{style:"currency",currency:balance.currency,currencyDisplay:"narrowSymbol"}).format(balance.balance!):balance?.state==="unchecked"?"—":balance?"Unavailable":"…";
 return <div className="account-balances"><span className="fal-balance" title={`${balance?.message??"Loading fal balance…"}${balance?.checkedAt?` Checked ${new Date(balance.checkedAt).toLocaleString()}.`:""}`}><span>fal</span><strong aria-live="polite">{amount}</strong>{balance?.state==="ready"&&<small>{balance.currency}</small>}</span>
  <button type="button" className="ai-balance-refresh" aria-label="Refresh fal balance" title="Refresh fal balance" disabled={busy} onClick={()=>void refresh(true)}><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 7a7 7 0 0 1 11.6-1L20 9M4 15l2.3 3A7 7 0 0 0 17.9 17"/></svg></button><AIBalances/></div>;
}
