use std::{net::{IpAddr,SocketAddr},sync::OnceLock,time::Duration};
use serde::Deserialize;
use tauri_plugin_updater::UpdaterBuilder;

pub type Routes=Vec<(&'static str,Vec<SocketAddr>)>;
#[derive(Deserialize)]
struct Answer { #[serde(rename="type")] kind:u16, data:String }
#[derive(Deserialize)]
struct Dns { #[serde(rename="Status")] status:u32, #[serde(rename="Answer",default)] answers:Vec<Answer> }

fn addresses(body:&[u8],kind:u16)->Result<Vec<SocketAddr>,String>{
    let dns:Dns=serde_json::from_slice(body).map_err(|e|e.to_string())?;
    if dns.status!=0{return Err("La consulta DNS no pudo completarse".into());}
    Ok(dns.answers.into_iter().filter(|a|a.kind==kind).filter_map(|a|a.data.parse::<IpAddr>().ok())
        .filter(|ip|!ip.is_loopback()&&!ip.is_unspecified())
        .map(|ip|SocketAddr::new(ip,443)).collect())
}
async fn lookup(name:&str,kind:u16)->Result<Vec<SocketAddr>,String>{
    static CLIENT:OnceLock<reqwest::Client>=OnceLock::new();
    let client=CLIENT.get_or_init(||reqwest::Client::builder().connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5)).user_agent("Whispera-Updater").build().expect("HTTPS client"));
    let mut last=String::new();
    for base in ["https://cloudflare-dns.com/dns-query","https://dns.google/resolve"]{
        let result=async {
            let response=client.get(base).query(&[("name",name),("type",&kind.to_string())])
                .header("Accept","application/dns-json").send().await.map_err(|e|e.to_string())?
                .error_for_status().map_err(|e|e.to_string())?;
            let bytes=response.bytes().await.map_err(|e|e.to_string())?;
            if bytes.len()>32768{return Err("Respuesta DNS demasiado grande".into());}
            addresses(&bytes,kind)
        }.await;
        match result {Ok(ips)=>return Ok(ips),Err(e)=>last=e}
    }
    Err(last)
}
pub async fn recovery_routes()->Result<Routes,String>{
    let (github,cdn_v6,cdn_v4)=tokio::join!(lookup("github.com",1),lookup("raw.githubusercontent.com",28),lookup("release-assets.githubusercontent.com",1));
    let github=github?;
    let mut cdn=cdn_v6.unwrap_or_default();
    cdn.extend(cdn_v4.unwrap_or_default());
    if github.is_empty()||cdn.is_empty(){return Err("No se pudo resolver la ruta alternativa de GitHub".into());}
    // GitHub's shared CDN also serves release assets over its IPv6 edge. Keep the
    // original URL, Host/SNI and certificate validation; never disable TLS or signatures.
    // Resolve fresh addresses instead of pinning geographic IPs or changing Windows DNS.
    Ok(vec![("github.com",github),("release-assets.githubusercontent.com",cdn)])
}
pub fn configure(builder:UpdaterBuilder,routes:Routes)->UpdaterBuilder{
    builder.configure_client(move|mut client|{
        client=client.connect_timeout(Duration::from_secs(5));
        for (host,ips) in &routes{client=client.resolve_to_addrs(host,ips);}
        client
    })
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn dns_records_are_typed_and_malformed_answers_are_rejected(){
        let ips=addresses(br#"{"Status":0,"Answer":[{"type":5,"data":"alias.example"},{"type":28,"data":"2606:50c0:8000::154"},{"type":1,"data":"127.0.0.1"}]}"#,28).unwrap();
        assert_eq!(ips.len(),1);assert!(ips[0].is_ipv6());
        assert!(addresses(br#"{"Status":2}"#,1).is_err());
        assert!(addresses(b"invalid",1).is_err());
    }
}
