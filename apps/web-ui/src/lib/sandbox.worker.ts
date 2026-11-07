import { newQuickJSWASMModuleFromVariant } from 'quickjs-emscripten-core';
import variant from '@jitl/quickjs-singlefile-browser-release-sync';

// User code executes inside QuickJS/WASM, never in the worker's host JS context.
// Only bounded console output crosses that boundary. No modules, host APIs or files.
self.onmessage=async(event:MessageEvent<string>)=>{
  if(typeof event.data!=='string'||event.data.length>50000){self.postMessage({error:'Maximum 50,000 characters of code.'});return;}
  try{
    const engine=await newQuickJSWASMModuleFromVariant(variant);
    self.postMessage({ready:true});
    const runtime=engine.newRuntime();runtime.setMemoryLimit(16*1024*1024);runtime.setMaxStackSize(256*1024);
    const deadline=Date.now()+1500;runtime.setInterruptHandler(()=>Date.now()>deadline);
    const vm=runtime.newContext();let output='';
    try{
      const log=vm.newFunction('log',(...args)=>{if(output.length<16000)output+=args.map(a=>String(vm.dump(a))).join(' ').slice(0,2000)+'\n';});
      const consoleObject=vm.newObject();for(const key of ['log','warn','error'])vm.setProp(consoleObject,key,log);vm.setProp(vm.global,'console',consoleObject);consoleObject.dispose();log.dispose();
      const result=vm.evalCode(event.data,'snippet.js',{type:'global'});
      if(result.error){const error=vm.dump(result.error);result.error.dispose();self.postMessage({output,error:typeof error==='object'?`${error.name}: ${error.message}`:String(error)});}
      else{const value=vm.dump(result.value);result.value.dispose();self.postMessage({output,result:String(value).slice(0,4000)});}
    }finally{vm.dispose();runtime.dispose();}
  }catch(error){self.postMessage({error:String(error)});}
};
