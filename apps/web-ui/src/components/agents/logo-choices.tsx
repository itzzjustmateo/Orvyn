"use client";
import {logoProposals} from "@/lib/logo-proposals";
export function LogoChoices({selected,onSelect}:{selected?:string;onSelect:(id:string)=>void}){
const chosen=logoProposals.find(logo=>logo.id===selected);
return <section aria-label="Logo-ontwerpen" className="space-y-5">
<div><p className="text-xs uppercase tracking-widest text-teal-300">Orvyn / design studio</p><h3 className="mt-2 text-xl font-semibold">Five directions. Your choice.</h3><p className="mt-2 text-sm text-gray-400">Preview the designs at large and small sizes. Selecting saves your preference only. The app and taskbar logos change only after your explicit approval.</p></div>
<div className="flex items-center gap-3 rounded-xl border border-white/10 p-3"><img src="/orvyn-logo.png" alt="Current Orvyn logo" className="size-10 object-contain"/><div><p className="text-sm">Current logo stays active</p><p className="text-xs text-gray-400">None of these designs has been applied.</p></div></div>
<fieldset><legend className="mb-3 text-sm text-gray-300">Choose a design to review</legend><div className="grid gap-3 sm:grid-cols-2">
{logoProposals.map((logo,i)=><label key={logo.id} className={"cursor-pointer rounded-2xl border p-4 transition-colors "+(selected===logo.id?"border-teal-300 bg-teal-300/5":"border-white/10 bg-white/[0.02] hover:border-white/30")}>
<div className="flex items-center justify-between"><span className="text-xs text-gray-500">0{i+1}</span><input type="radio" name="logo-proposal" aria-label={logo.name+" as preference"} value={logo.id} checked={selected===logo.id} onChange={()=>onSelect(logo.id)} className="accent-teal-300"/></div>
<div className="flex h-32 items-center justify-center gap-7"><img alt={logo.name+" logo-ontwerp"} src={"data:image/svg+xml,"+encodeURIComponent(logo.svg)} className="size-24"/><div className="space-y-3 border-l border-white/10 pl-6"><img alt="" src={"data:image/svg+xml,"+encodeURIComponent(logo.svg)} className="size-8"/><img alt="" src={"data:image/svg+xml,"+encodeURIComponent(logo.svg)} className="size-4"/></div></div>
<h4 className="font-semibold">{logo.name}</h4><p className="mt-1 text-xs leading-relaxed text-gray-400">{logo.description}</p>
</label>)}</div></fieldset>
<div role="status" className="rounded-xl bg-white/5 p-3 text-sm">{chosen?chosen.name+" is ready as a preference. Not approved or applied yet.":"No design selected yet."}</div>
{selected&&<button className="text-sm text-gray-400 underline" onClick={()=>onSelect("")}>Clear preference</button>}
</section>;
}
