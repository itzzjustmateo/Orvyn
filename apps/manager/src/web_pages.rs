use crate::core::{self,err,Result};
use serde_json::{json,Value};
use std::{fs,net::{Ipv4Addr,ToSocketAddrs},path::Path,sync::{Mutex,OnceLock},time::{Instant,Duration}};
use url::Url;
fn public(ip:Ipv4Addr)->bool{
 let [a,b,c,_]=ip.octets();
 !(a==0||a==10||a==127||a>=224||a==169&&b==254||a==172&&(16..=31).contains(&b)||a==192&&(b==168||b==0||b==2)||a==100&&(64..=127).contains(&b)||a==198&&(b==18||b==19||b==51&&c==100)||a==203&&b==0&&c==113)
}
pub fn validate(raw:&str)->Result<Url>{
 if raw.len()>4000{return Err("Webadres te lang".into());}
 let mut url=Url::parse(raw).map_err(err)?;
 if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.port_or_known_default()!=Some(443){return Err("Use a public HTTPS link without credentials.".into());}
 url.set_fragment(None);
 if url.host_str().is_none(){return Err("Host is missing".into());}
 Ok(url)
}
fn address(url:&Url)->Result<Ipv4Addr>{
 (url.host_str().unwrap(),443).to_socket_addrs().map_err(err)?.filter_map(|s|match s.ip(){std::net::IpAddr::V4(ip) if public(ip)=>Some(ip),_=>None}).next().ok_or("This website has no permitted public IPv4 address.".into())
}
fn decode(s:&str)->String{
 let mut out=String::new();let mut rest=s;
 while let Some(i)=rest.find('&'){
  out.push_str(&rest[..i]);rest=&rest[i..];
  if let Some(end)=rest.find(';').filter(|i|*i<14){
   let entity=&rest[1..end];
   let ch=match entity{"amp"=>Some('&'),"lt"=>Some('<'),"gt"=>Some('>'),"quot"=>Some('"'),"apos"|"#39"=>Some('\''),"nbsp"=>Some(' '),_=>entity.strip_prefix("#x").or_else(||entity.strip_prefix("#X")).and_then(|v|u32::from_str_radix(v,16).ok()).or_else(||entity.strip_prefix('#').and_then(|v|v.parse().ok())).and_then(char::from_u32)};
   if let Some(ch)=ch{out.push(ch);rest=&rest[end+1..];continue;}
  }
  out.push('&');rest=&rest[1..];
 }
 out.push_str(rest);out
}
fn extract(html:&str)->String{
 let lower=html.to_ascii_lowercase();let mut out=String::new();let mut i=0;
 while i<html.len(){
  if html.as_bytes()[i]==b'<'{
   if lower[i..].starts_with("<!--"){i=lower[i..].find("-->").map(|end|i+end+3).unwrap_or(html.len());continue;}
   let mut end=i+1;let mut quote=None;
   while end<html.len(){let c=html.as_bytes()[end];if let Some(q)=quote{if c==q{quote=None;}}else if c==b'\''||c==b'"'{quote=Some(c);}else if c==b'>'{break;}end+=1;}
   if end==html.len(){break;}
   let tag=lower[i+1..end].split_whitespace().next().unwrap_or("").trim_end_matches('/');
   if ["script","style","noscript","svg","template"].contains(&tag){
    let closing=format!("</{tag}");i=lower[end+1..].find(&closing).map(|n|end+1+n).unwrap_or(html.len());continue;
   }
   out.push(' ');i=end+1;
  }else{let c=html[i..].chars().next().unwrap();out.push(c);i+=c.len_utf8();}
  if out.len()>150_000{break;}
 }
 decode(&out).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(12000).collect()
}
type Cache=Vec<(String,Instant,Value)>;
pub fn read(root:&Path,data:&Value)->Result<Value>{
 static CACHE:OnceLock<Mutex<Cache>>=OnceLock::new();
 let mut url=validate(data["url"].as_str().ok_or("Web address is missing")?)?;
 let key=url.to_string();let cache=CACHE.get_or_init(||Mutex::new(vec![]));
 if let Some((_,_,value))=cache.lock().map_err(err)?.iter().find(|(u,t,_)|*u==key&&t.elapsed()<Duration::from_secs(300)){return Ok(value.clone());}
 let temp=root.join("tools/temp");fs::create_dir_all(&temp).map_err(err)?;
 let id=core::id();let body=temp.join(format!("page-{id}.html"));let headers=temp.join(format!("page-{id}.headers"));
 let result=(||{
  for _ in 0..4 {
   let ip=address(&url)?;let pinned=format!("{}:443:{ip}",url.host_str().unwrap());
   let output=core::hidden("curl.exe").args(["--silent","--show-error","--noproxy","*","--proto","=https","--connect-timeout","5","--max-time","15","--max-filesize","2000000","--resolve",&pinned,"-A","Orvyn/1 (web page reader)","-D"]).arg(&headers).arg("-o").arg(&body).args(["-w","%{http_code}","--url",url.as_str()]).output().map_err(err)?;
   if !output.status.success(){return Err("Website could not be read (network error, timeout, or page too large).".into());}
   let status=String::from_utf8_lossy(&output.stdout).trim().parse::<u16>().map_err(err)?;
   let head=fs::read_to_string(&headers).map_err(err)?;
   let header=|name:&str|head.lines().filter_map(|line|line.split_once(':')).filter(|(k,_)|k.eq_ignore_ascii_case(name)).last().map(|(_,v)|v.trim().to_string());
   if (300..400).contains(&status){url=validate(url.join(&header("location").ok_or("Redirect zonder locatie")?).map_err(err)?.as_str())?;continue;}
   if status!=200{return Err(format!("Website returned HTTP {status}. Sign-in requirements and access restrictions are not bypassed."));}
   let mime=header("content-type").unwrap_or_default().to_lowercase();
   if !mime.starts_with("text/html")&&!mime.starts_with("text/plain")&&!mime.starts_with("application/xhtml+xml"){return Err("Only HTML and text pages are supported.".into());}
   let bytes=fs::read(&body).map_err(err)?;if bytes.len()>2_000_000{return Err("Page te groot".into());}
   let html=String::from_utf8_lossy(&bytes);
   let text=if mime.starts_with("text/plain"){html.chars().take(12000).collect::<String>()}else{extract(&html)};
   if text.trim().len()<30{return Err("No usable page text. The page may require JavaScript or sign-in.".into());}
   return Ok(json!({"name":url.host_str().unwrap(),"url":url.as_str(),"text":text}));
  }
  Err("Te veel doorverwijzingen".into())
 })();
 let _=fs::remove_file(body);let _=fs::remove_file(headers);
 if let Ok(ref value)=result{let mut c=cache.lock().map_err(err)?;c.retain(|(_,t,_)|t.elapsed()<Duration::from_secs(300));if c.len()>=16{c.remove(0);}c.push((key,Instant::now(),value.clone()));}
 result
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn blocks_local_and_unsafe_urls(){for raw in ["file:///C:/secret","http://example.com","https://user:pass@example.com","https://example.com:8080"]{assert!(validate(raw).is_err());}for ip in ["127.0.0.1","10.0.0.1","192.168.1.1","169.254.169.254","100.64.0.1","0.0.0.0","198.18.0.1"]{assert!(!public(ip.parse().unwrap()));}assert!(public("93.184.216.34".parse().unwrap()));}
 #[test]fn extracts_text_without_scripts(){assert_eq!(extract("<h1>Titel &amp; uitleg</h1><script>ignore me</script><style>hidden</style><!-- hidden --><p title='x > y'>Caf&#233; &#x41;</p>"),"Titel & uitleg Café A");}
}
