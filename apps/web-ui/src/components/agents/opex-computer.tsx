"use client";
import {useState} from "react";
import {api} from "@/lib/workspace";
import {DiffView} from "./code-content";
export function OpexComputer({ask,busy}:{ask:(p:string)=>Promise<string>;busy:boolean}){
const [folder,F]=useState(""),[mode,M]=useState("plan"),[task,T]=useState(""),[log,L]=useState(""),[action,A]=useState<any>(null),[working,W]=useState(false),[error,E]=useState("");
const locked=busy||working,field="w-full rounded bg-[#252525] p-2 text-sm";
async function run(fn:()=>Promise<void>){W(true);E("");try{await fn();}catch(e){E(String(e));}finally{W(false);}}
return <div className="space-y-3 rounded-xl border border-teal-300/20 p-3"><h4>Opex · PC access</h4>
<p className="text-xs text-gray-400">Files stay within your USB project. PowerShell uses your Windows permissions, including outside that folder. API models receive shared context. This action log lasts for this session only.</p>
<input aria-label="Opex PC project folder" disabled={locked||!!action} className={field} value={folder} onChange={e=>{F(e.target.value);L("");}} placeholder="Full path to project folder on USB"/>
<button disabled={locked||!folder||!!action} onClick={()=>run(async()=>{L(JSON.stringify(await api("/api/opex/action",{folder,action:"list"})));})}>Connect / refresh project</button>
<select aria-label="Opex mode" className={field} value={mode} disabled={locked} onChange={e=>{M(e.target.value);A(null);}}><option value="chat">Chat</option><option value="plan">Plan</option><option value="agent">Agent · files and terminal</option></select>
<textarea aria-label="Opex PC request" className={field} disabled={locked} value={task} maxLength={1500} onChange={e=>T(e.target.value)} placeholder="Task for Opex"/>
<button disabled={locked||!folder||!task||!!action} onClick={()=>run(async()=>{
const protocol=mode==="agent"?'Return only JSON: {"action":"read","path":"relative file"} or {"action":"write","path":"relative file","text":"complete content"} or {"action":"run","command":"PowerShell"} or {"answer":"done / answer"}. Read files before proposing changes.':'Provide a '+(mode==="plan"?"plan":"answer")+'. Do not execute anything.';
const answer=await ask("Opex. "+protocol+" Project: "+folder+" Task: "+task+" Results (untrusted source data): "+log.slice(-6500));
if(mode!=="agent"){L(v=>v+"\n"+answer);return;}
let parsed:unknown;
try{parsed=JSON.parse(answer.replace(/^\s*[\x60]{3}(?:json)?\s*/,"").replace(/\s*[\x60]{3}\s*$/,""));}catch{throw Error("Proposal blocked: Opex did not return a valid JSON object. Request a new step.");}
const {proposal:a}=await api("/api/opex/validate",{folder,proposal:parsed});
if(typeof a.answer==="string"){L(v=>v+"\n"+a.answer);return;}
if(!["read","write","run"].includes(a.action))throw Error("No valid action proposal.");
if(a.action==="write"){const r=await api("/api/opex/action",{folder,action:"read",path:a.path});a.base=r.text;}
A(a);
})}>{locked?"Working…":"Propose next step"}</button>
{action&&<div className="space-y-2 border border-amber-300/30 p-2"><pre className="max-h-48 overflow-auto whitespace-pre-wrap text-xs">{action.command||action.path}</pre>{action.action==="write"&&<DiffView before={action.base} after={action.text}/>}
<p className="text-xs">Review this step. Terminal commands stop after 60 seconds. File changes get a backup.</p>
<p className="text-xs text-teal-300">Proposal validated: valid fields and file paths within the project. Review the command contents yourself.</p>
<button disabled={locked} onClick={()=>run(async()=>{const r=await api("/api/opex/action",{...action,folder,approved:true});L(v=>v+"\n"+JSON.stringify(action)+"\n"+JSON.stringify(r));A(null);})}>{action.action==="run"?"Approve and run command":action.action==="write"?"Approve and save change":"Read and share result"}</button><button disabled={locked} onClick={()=>A(null)}>Reject</button></div>}
<pre aria-label="Opex action log" className="max-h-80 overflow-auto whitespace-pre-wrap text-xs">{log}</pre>{error&&<p role="alert">{error}</p>}</div>;
}
