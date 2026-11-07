//! The original React/CSS interface, served locally by Rust and hosted by WebView2.
use crate::core::{self, err, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use tiny_http::{Header, Response, Server, StatusCode};

fn json_response(value: Value, status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(value.to_string().into_bytes())
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
}
struct EventReader {rx:std::sync::mpsc::Receiver<String>,buffer:std::io::Cursor<Vec<u8>>}
impl Read for EventReader{
    fn read(&mut self,out:&mut[u8])->std::io::Result<usize>{if out.is_empty(){return Ok(0);}loop{let n=self.buffer.read(out)?;if n>0{return Ok(n);}match self.rx.recv_timeout(Duration::from_secs(5)){Ok(value)=>self.buffer=std::io::Cursor::new(value.into_bytes()),Err(std::sync::mpsc::RecvTimeoutError::Timeout)=>self.buffer=std::io::Cursor::new(b": keepalive\n\n".to_vec()),Err(_)=>return Ok(0)}}}
}
pub fn run(root: PathBuf, smoke: bool) -> Result<()> {
    use std::hash::{Hash,Hasher};
    let mut hash=std::collections::hash_map::DefaultHasher::new();root.hash(&mut hash);
    let _instance=core::Lock::acquire(&format!("Local\\AIUSB-Desktop-{}",hash.finish()))?;
    let _job=core::ProcessJob::new()?;
    let started=Instant::now();
    let startup=Arc::new(crate::diagnostics::Startup::new());
    let cfg = core::config(&root)?;
    startup.mark("configuration","Reading configuration");
    let runtime=crate::models::Runtime::with_config(root.clone(),&cfg)?;
    startup.mark("runtime","Preparing model manager");
    let assets = core::path(&cfg, "web_ui");
    if !assets.join("index.html").exists() {
        return Err("The bundled web interface is missing (apps/web-ui/dist).".into());
    }
    let server = Server::http("127.0.0.1:0").map_err(err)?;
    let base = format!("http://{}", server.server_addr());
    let token = core::id();
    let session = core::path(&cfg, "data").join("desktop-session.json");
    core::atomic_transient(
        &session,
        &json!({"url":base,"token":token,"ui_loaded":false}),
    )?;
    startup.mark("server","Preparing local connection");
    let lock = Arc::new(Mutex::new(()));
    let server_root = root.clone();
    let server_cfg=Arc::new(cfg.clone());
    let server_token = token.clone();
    let server_runtime=runtime.clone();
    let server_startup=startup.clone();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let root = server_root.clone();
            let request_cfg=server_cfg.clone();
            let assets = assets.clone();
            let token = server_token.clone();
            let lock = lock.clone();
            let runtime=server_runtime.clone();
            let startup=server_startup.clone();
            std::thread::spawn(move || {
                let url = request.url().split('?').next().unwrap_or("/").to_owned();
                if url.starts_with("/api/") {
                    if !request
                        .headers()
                        .iter()
                        .any(|h| h.field.equiv("X-AIUSB-Session") && h.value.as_str() == token)
                    {
                        let _ = request
                            .respond(json_response(json!({"error":"Invalid session"}), 403));
                        return;
                    }
                    if url=="/api/chat-stream"&&request.method().as_str()=="POST"{
                        let parsed=(||->Result<Value>{let mut bytes=vec![];request.as_reader().take(40_000_001).read_to_end(&mut bytes).map_err(err)?;if bytes.len()>40_000_000{return Err("Aanvraag te groot".into());}serde_json::from_slice(&bytes).map_err(err)})();
                        let data=match parsed{Ok(v)=>v,Err(e)=>{let _=request.respond(json_response(json!({"error":e}),400));return;}};
                        let (tx,rx)=std::sync::mpsc::channel();
                        std::thread::spawn(move||{
                            let key=data["model"].as_str().unwrap_or("").to_string();
                            let result=runtime.stream(&key,data,|event|tx.send(event).map_err(err));
                            if let Err(error)=result{let _=tx.send(format!("data: {}\n\n",json!({"error":error})));}
                            let _=tx.send("data: [DONE]\n\n".into());
                        });
                        let response=Response::new(StatusCode(200),vec![Header::from_bytes("Content-Type","text/event-stream; charset=utf-8").unwrap(),Header::from_bytes("Cache-Control","no-cache").unwrap()],EventReader{rx,buffer:std::io::Cursor::new(vec![])},None,None).with_chunked_threshold(1);
                        let _=request.respond(response);return;
                    }
                    let result = (|| -> Result<(Value, u16)> {
                        let cfg = request_cfg;
                        if url == "/api/models" {
                            return Ok((runtime.list()?, 200));
                        }
                        if url=="/api/catalog"&&request.method().as_str()=="GET" {return Ok((runtime.overview()?,200));}
                        if request.method().as_str()=="GET" {
                            if url=="/api/diagnostics/startup"{return Ok((startup.snapshot(),200));}
                            if url=="/api/training"{return Ok((crate::training::overview(&root)?,200));}
                            if url=="/api/providers"{return Ok((runtime.providers.list()?,200));}
                            if url=="/api/hardware"{return Ok((crate::workspace::hardware()?,200));}
                            if url=="/api/disk-usage"{return Ok((crate::workspace::storage(&root),200));}
                            if url=="/api/documents"{return Ok((crate::workspace::documents(&root)?,200));}
                        }
                        if url == "/api/storage" && request.method().as_str() == "GET" {
                            let file = core::path(&cfg, "data").join("chats.json");
                            return Ok((
                                if file.exists() {
                                    core::read_json(&file)?
                                } else {
                                    Value::Null
                                },
                                200,
                            ));
                        }
                        if request.body_length().unwrap_or(0) > 40_000_000 {
                            return Ok((json!({"error":"Aanvraag te groot"}), 413));
                        }
                        let mut body = Vec::new();
                        request
                            .as_reader()
                            .take(40_000_001)
                            .read_to_end(&mut body)
                            .map_err(err)?;
                        if body.len() > 40_000_000 {
                            return Ok((json!({"error":"Aanvraag te groot"}), 413));
                        }
                        let mut data: Value = serde_json::from_slice(&body).map_err(err)?;
                        if request.method().as_str()=="POST" {
                            if url=="/api/training/candidate"{return Ok((crate::training::candidate(&root,&data)?,200));}
                            if url=="/api/training/examples"{return Ok((crate::training::examples(&root,&data)?,200));}
                            if url=="/api/providers"{return Ok((runtime.providers.save(&data)?,200));}
                            if url=="/api/providers/remove"{return Ok((runtime.providers.remove(&data)?,200));}
                            if url=="/api/providers/models"{return Ok((runtime.providers.models(&data)?,200));}
                            if url=="/api/documents"{return Ok((crate::workspace::import_document(&root,&data)?,201));}
                            if url=="/api/documents/get"{return Ok((crate::workspace::document(&root,&data)?,200));}
                            if url=="/api/backup"{return Ok((json!({"path":core::backup(&core::config(&root)?)?}),201));}
                            if url=="/api/opex/action"{return Ok((crate::opex::action(&root,&data)?,200));}
                            if url=="/api/opex/tree"{return Ok((crate::opex::tree(&root,&data)?,200));}
                            if url=="/api/opex/run"{return Ok((crate::opex::start_job(&root,&data)?,202));}
                            if url=="/api/opex/job"{return Ok((crate::opex::job(&data,false)?,200));}
                            if url=="/api/opex/stop"{return Ok((crate::opex::job(&data,true)?,200));}
                            if url=="/api/opex/validate"{return Ok(match crate::opex::validate(&root,&data){Ok(v)=>(v,200),Err(e)=>(json!({"error":e,"valid":false}),400)});}
                            if url=="/api/opex/export"{return Ok((crate::workspace::export_code(&root,&data)?,201));}
                            if url=="/api/export"{return Ok((crate::workspace::export(&root,&data)?,201));}
                            if url=="/api/index/folder"{return Ok((crate::workspace::index_folder(&root,&data)?,200));}
                            if url=="/api/index/scan"{return Ok((crate::workspace::scan_index(&root)?,200));}
                            if url=="/api/documents/search"{return Ok((crate::workspace::search(&root,&data)?,200));}
                            if url=="/api/web-read"{return Ok((crate::web_pages::read(&root,&data)?,200));}
                            if url=="/api/web-search"{return Ok((crate::workspace::web_search(&data)?,200));}
                            if url=="/api/speech"{return Ok((crate::workspace::speech(&data)?,200));}
                            if url=="/api/open-url"{return Ok((crate::workspace::open_url(&data)?,200));}
                            if url=="/api/models/custom"{let id=crate::custom_models::remote(&root,&data)?;return Ok((json!({"id":id}),201));}
                            if url=="/api/models/store"{return Ok((runtime.store_search(&data)?,200));}
                            if url=="/api/models/import"{let id=runtime.import_model(&data)?;return Ok((json!({"id":id}),202));}
                            if url=="/api/models/clear-selection"{let slot=data["slot"].as_str().unwrap_or("model");if ["model","projector"].contains(&slot){runtime.select_file(slot,None);}return Ok((json!({"ok":true}),200));}
                            if url=="/api/models/download"{runtime.enqueue(data["id"].as_str().ok_or("Model ID is missing")?)?;return Ok((json!({"ok":true}),202));}
                            if url=="/api/models/queue"{runtime.edit_queue(&data)?;return Ok((json!({"ok":true}),200));}
                            if url=="/api/models/pause"{runtime.pause_download();return Ok((json!({"ok":true}),200));}
                            if url=="/api/models/unload"{return Ok((json!({"message":runtime.unload()?}),200));}
                            if url=="/api/models/cancel"{runtime.cancel_answer()?;return Ok((json!({"ok":true}),200));}
                        }
                        if url == "/api/storage" && request.method().as_str() == "PUT" {
                            if !data["chats"].as_array().is_some_and(|a| {
                                a.iter()
                                    .all(|v| v["id"].is_string() && v["messages"].is_array())
                            }) {
                                return Ok((json!({"error":"Invalid conversations"}), 400));
                            }
                            let _guard = lock.lock().map_err(err)?;
                            match core::save_chats(&cfg, &mut data) {
                                Ok(()) => return Ok((json!({"revision":data["revision"]}), 200)),
                                Err(e) => return Ok((json!({"error":e}), 409)),
                            }
                        }
                        if url == "/api/chat" && request.method().as_str() == "POST" {
                            let cfg=core::config(&root)?;
                            let requested=data["model"].as_str().unwrap_or("").to_string();
                            if requested.starts_with("api-"){return Ok((runtime.chat(&requested,data)?,200));}
                            let name = requested.to_lowercase();
                            let key=cfg["services"].as_object().unwrap().iter().find(|(key,s)|key.to_lowercase()==name||s["name"].as_str().unwrap_or("").to_lowercase()==name).map(|(key,_)|key.clone()).ok_or("This model is unavailable")?;
                            let response = runtime.chat(&key, data)?;
                            return Ok((response, 200));
                        }
                        Ok((json!({"error":"Onbekende route"}), 404))
                    })();
                    let response = match result {
                        Ok((data, status)) => json_response(data, status),
                        Err(e) => json_response(json!({"error":e}), 500),
                    };
                    let _ = request.respond(response);
                } else {
                    let (file, mime) = match url.as_str() {
                        "/" | "/index.html" => ("index.html", "text/html; charset=utf-8"),
                        "/orvyn-logo.png" => ("orvyn-logo.png", "image/png"),
                        "/app.js" => ("app.js", "text/javascript; charset=utf-8"),
                         "/app.css" => ("app.css", "text/css; charset=utf-8"),
                          "/pdf.worker.min.mjs" => ("pdf.worker.min.mjs", "text/javascript; charset=utf-8"),
                          "/sandbox.worker.js" => ("sandbox.worker.js", "text/javascript; charset=utf-8"),
                        _ if url.starts_with("/chunks/")&&url.ends_with(".js")&&url[8..].bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)) => (&url[1..], "text/javascript; charset=utf-8"),
                        _ => {
                            let _ = request.respond(Response::empty(StatusCode(404)));
                            return;
                        }
                    };
                    match fs::read(assets.join(file)) {
                        Ok(bytes) => {
                            let _ = request
                                .respond(Response::from_data(bytes).with_header(
                                    Header::from_bytes("Content-Type", mime).unwrap(),
                                ));
                        }
                        Err(e) => {
                            let _ =
                                request.respond(json_response(json!({"error":e.to_string()}), 500));
                        }
                    }
                }
            });
        }
    });
    use tao::platform::windows::WindowBuilderExtWindows;
    let icon=tao::window::Icon::from_rgba(include_bytes!("../assets/orvyn.rgba").to_vec(),256,256).map_err(err)?;
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Orvyn · AI Model Hub")
        .with_window_icon(Some(icon.clone()))
        .with_taskbar_icon(Some(icon))
        .with_inner_size(LogicalSize::new(1200., 820.))
        .with_min_inner_size(LogicalSize::new(800., 600.))
        .build(&event_loop)
        .map_err(err)?;
    let window_ms=started.elapsed().as_millis();
    startup.mark("window","Creating application window");
    // Profile and browser cache belong to the USB, not AppData on C:.
    let profile = core::path(&cfg, "data").join("webview-profile");
    fs::create_dir_all(&profile).map_err(err)?;
    let mut context = wry::WebContext::new(Some(profile));
    let js = format!(
        r#"(()=>{{
      const fetchOriginal=window.fetch.bind(window);
      window.fetch=(input,init={{}})=>{{const url=new URL(typeof input==='string'?input:input.url,location.href);if(url.origin===location.origin&&url.pathname.startsWith('/api/')){{init.headers=new Headers(init.headers||{{}});init.headers.set('X-AIUSB-Session',{});}}return fetchOriginal(input,init);}};
      window.addEventListener('keydown',e=>{{if(e.ctrlKey&&e.shiftKey&&e.code==='KeyM'){{e.preventDefault();window.ipc.postMessage('manage');}}}});
      window.addEventListener('DOMContentLoaded',()=>{{let sent=false;const mark=()=>{{if(sent||!document.querySelector('[aria-label="Chat message"]'))return;sent=true;observer.disconnect();requestAnimationFrame(()=>requestAnimationFrame(()=>{{const nav=performance.getEntriesByType('navigation')[0];const storage=performance.getEntriesByName('orvyn-storage')[0];const resources=performance.getEntriesByType('resource').filter(r=>r.name.startsWith(location.origin)).sort((a,b)=>b.duration-a.duration).slice(0,8).map(r=>({{path:new URL(r.name).pathname,duration_ms:Math.round(r.duration)}}));window.ipc.postMessage(JSON.stringify({{type:'ready',textarea:true,background:getComputedStyle(document.body).backgroundColor,frontend:{{dom_ms:Math.round(nav?.domContentLoadedEventEnd||0),storage_ms:storage?Math.round(storage.duration):null,ready_ms:Math.round(performance.now()),resources}}}}));}}));}};const observer=new MutationObserver(mark);observer.observe(document.documentElement,{{childList:true,subtree:true}});mark();}});
    }})();"#,
        serde_json::to_string(&token).unwrap()
    );
    let ipc_session = session.clone();
    let ipc_base = base.clone();
    let ipc_token = token.clone();
    let allowed = base.clone();
    let picker_runtime=runtime.clone();
    let ipc_startup=startup.clone();
    let _webview=wry::WebViewBuilder::new_with_web_context(&mut context)
        .with_background_color((33,33,33,255)).with_url(format!("{base}/"))
        .with_initialization_script(js)
        .with_navigation_handler(move|url|url==format!("{allowed}/")||url.starts_with(&format!("{allowed}/#")))
        .with_ipc_handler(move|request|{
            if request.body()=="manage"{if let Ok(exe)=std::env::current_exe(){let _=core::hidden(&exe.to_string_lossy()).arg("manage").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();}}
            else if let Ok(info)=serde_json::from_str::<Value>(request.body()){
                if info["action"]=="pick-gguf"{if let Some(slot)=info["slot"].as_str(){if ["model","projector"].contains(&slot){if let Some(file)=crate::window::pick_model_file(){picker_runtime.select_file(slot,Some(file));}}}}
                else if info["type"]=="ready"{
                    ipc_startup.ready(&info["frontend"]);
                    let path=ipc_session.clone();
                    let status=json!({"url":ipc_base,"token":ipc_token,"ui_loaded":info["textarea"],"background":info["background"],"startup":{"window_ms":window_ms,"ready_ms":started.elapsed().as_millis()}});
                    // Flushing a USB can stall for seconds; keep it off the window's event loop.
                    std::thread::spawn(move||{let _=core::atomic_transient(&path,&status);});
                }
            }
        }).build(&window).map_err(err)?;
    startup.mark("webview","Preparing WebView");
    let deadline = Instant::now() + Duration::from_secs(10);
    event_loop.run(move |event, _, flow| {
        *flow = if smoke {
            ControlFlow::WaitUntil(deadline)
        } else {
            ControlFlow::Wait
        };
        if smoke && Instant::now() >= deadline {
            runtime.shutdown();
            *flow = ControlFlow::Exit;
        }
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            runtime.shutdown();
            *flow = ControlFlow::Exit;
        }
    })
}
