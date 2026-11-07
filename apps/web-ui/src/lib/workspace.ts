import { modelQuality } from "@/components/agents/model-quality";

export type Hardware = {ram_bytes:number;available_bytes:number;cpu:string;threads:number;model_ram_bytes:number};
export type Preferences = {logoProposal?:string;chatMode?:"chat"|"plan"|"code";fontSize?:number;highContrast?:boolean;reduceMotion?:boolean;enterToSend?:boolean;codeInstructions?:string;autoModel:boolean;style:string;language:string;favorites:string[];web:boolean;documents:boolean;semantic?:boolean;collection?:string};
export const defaultPreferences:Preferences={fontSize:16,highContrast:false,reduceMotion:false,enterToSend:true,codeInstructions:"",autoModel:false,style:"normaal",language:"English",favorites:[],web:false,documents:false};
export type ModelInfo={id:string;category:string;installed:boolean;bytes:number;name:string;custom?:boolean;remote?:boolean};
export type Source={name:string;text:string;url?:string;section?:number;id?:string;page?:number;pdf?:boolean};
export const gb=(bytes:number)=>`${(bytes/1e9).toFixed(2)} GB`;
export async function api(path:string,body?:unknown){
  const response=await fetch(path,body===undefined?undefined:{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(body)});
  const data=await response.json();if(!response.ok||data.error)throw new Error(typeof data.error==="string"?data.error:"Request failed");return data;
}
export function scores(model:ModelInfo){
  const quality=model.custom?undefined:modelQuality[model.id];if(quality===undefined)return null;
  return {Dutch:Math.max(1,quality-(model.id.startsWith("mimo")||model.id==="smollm2"?1:0)),Code:Math.max(1,quality-(model.category==="code"?0:1)),Reasoning:Math.max(1,quality-(model.category==="reasoning"?0:1)),Speed:model.bytes>7e9?1:model.bytes>4e9?2:model.bytes>2e9?3:model.bytes>1e9?4:5};
}
export function requiredRam(model:ModelInfo){return model.bytes>0?model.bytes*1.25+1.5e9:0;}
export function recommend(model:ModelInfo,hardware:Hardware|null){
  const needed=requiredRam(model);if(!hardware||!needed)return "Memory recommendation not available yet";
  return `${needed<hardware.available_bytes?"Likely fits in free RAM":needed<hardware.ram_bytes*.8?"Close other apps to free memory":"Demanding for this PC"} · approximately ${gb(needed)} RAM · ${hardware.threads<=4&&model.bytes>4e9?"likely slow on this CPU":"CPU speed depends on the model"}`;
}
export function chooseModel(text:string,models:ModelInfo[],hardware:Hardware|null,image=false){
  const category=image?"vision":/\b(code|python|javascript|typescript|rust|programming|function|programmeer|functie|bug|sql|html|css)\b/i.test(text)?"code":/\b(calculate|math|proof|logic|reason|compare|analyze|reken|bereken|wiskunde|bewijs|logica|redeneer|vergelijk|analyseer)\b/i.test(text)?"reasoning":"chat";
  const installed=models.filter(m=>!m.remote&&m.installed&&(image?m.category==="vision":m.category!=="vision"));
  const fitting=hardware?installed.filter(m=>requiredRam(m)>0&&requiredRam(m)<hardware.ram_bytes*.8):installed;
  return (fitting.length?fitting:installed).sort((a,b)=>((b.category===category?10:0)+(modelQuality[b.id]||0))-((a.category===category?10:0)+(modelQuality[a.id]||0))||a.bytes-b.bytes)[0];
}
export async function readDocument(file:File){
  if(file.size>20_000_000)throw new Error("Maximum 20 MB per document.");
  if(file.name.toLowerCase().endsWith(".pdf")){
    const pdfjs=await import("pdfjs-dist");pdfjs.GlobalWorkerOptions.workerSrc="/pdf.worker.min.mjs";
    const task=pdfjs.getDocument({data:await file.arrayBuffer(),isEvalSupported:false});
    const pdf=await task.promise;let text="";
    try{for(let page=1;page<=pdf.numPages;page++){const p=await pdf.getPage(page);const content=await p.getTextContent();let extracted=content.items.map(item=>"str" in item?item.str:"").join(" ");if(!extracted.trim()){
      if(pdf.numPages>30)throw new Error('Split scanned PDFs into batches of up to 30 pages for OCR.');
      const catalog=await api('/api/catalog');const models=Object.entries(catalog.models) as [string,{category:string;installed:boolean;remote?:boolean}][];const vision=models.find(([id,m])=>id==='glm-ocr'&&m.installed&&!m.remote)||models.find(([,m])=>m.category==='vision'&&m.installed&&!m.remote);if(!vision)throw new Error('Install a vision model to run OCR on this scanned PDF.');
      const viewport=p.getViewport({scale:1.25}),canvas=document.createElement('canvas');canvas.width=viewport.width;canvas.height=viewport.height;await p.render({canvasContext:canvas.getContext('2d')!,viewport}).promise;
      const answer=await api('/api/chat',{model:vision[0],messages:[{role:'user',content:[{type:'text',text:'Transcribe all text on this page verbatim. Return only the detected text.'},{type:'image_url',image_url:{url:canvas.toDataURL('image/png')}}]}],max_tokens:2048});extracted=answer.choices?.[0]?.message?.content||'';
    }text+=`\n[Page ${page}]\n`+extracted;if(text.length>1_000_000)throw new Error("PDF bevat te veel tekst.");}}
    finally{await pdf.destroy();}
    if(!text.replace(/\[Page \d+\]/g,"").trim())throw new Error("This PDF contains no extractable text. Use OCR for scanned pages.");
    return text;
  }
  if(/\.docx$/i.test(file.name)){const module=await import('mammoth');const mammoth=module.default||module;return (await mammoth.extractRawText({arrayBuffer:await file.arrayBuffer()})).value;}
  if(/\.(xlsx|xls|csv)$/i.test(file.name)){const xlsx=await import('xlsx');const book=xlsx.read(await file.arrayBuffer(),{type:'array',sheetRows:2000});return book.SheetNames.map(name=>`[Worksheet ${name}; up to the first 2000 rows]\n${xlsx.utils.sheet_to_csv(book.Sheets[name])}`).join('\n\n');}
  if(!/\.(txt|md|json|log|py|js|jsx|ts|tsx|rs|html|css|java|c|cpp|h|go|sql|yaml|yml|toml)$/i.test(file.name))throw new Error("Choose PDF, Word, Excel, text, or a supported code file.");
  return file.text();
}
export function base64(bytes:ArrayBuffer){let text='';const array=new Uint8Array(bytes);for(let i=0;i<array.length;i+=8192)text+=String.fromCharCode(...array.subarray(i,i+8192));return btoa(text);}
export async function importDocument(file:File,metadata:Record<string,unknown>={}){return api('/api/documents',{name:file.name,text:await readDocument(file),...metadata,...(/\.pdf$/i.test(file.name)?{pdf:base64(await file.arrayBuffer())}:{})});}
export function preferenceInstruction(p:Preferences){const mode=p.chatMode==="plan"?"Provide a concrete plan. Do not execute anything.":p.chatMode==="code"?"Help with programming. Do not claim tests you have not run.":"Answer conversationally.";return mode+" "+ `Response language: ${p.language}. ${p.style==="kort"?"Be concise.":p.style==="eenvoudig"?"Use simple words and explain concepts.":p.style==="technisch"?"Give precise technical details.":p.style==="uitgebreid"?"Give a full explanation with examples.":"Use a balanced response style."}`;}
