//! A SOCKS5 egress: the host name goes to the proxy to be resolved there, so the lookup does not
//! leave from this machine, and the credentials travel in the SOCKS handshake.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};

use seleniumbase_rs::impersonate::mitm::MitmOptions;
use seleniumbase_rs::scrape::egress::route::EgressRef;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::support::{der, exchange, measured_user_agent, navigation, new_ca, start_mitm, tunnel};

/// The site the browser asks for; no resolver knows it.
const NAME: &str = "far-away.test";
/// The SOCKS5 address type of an IPv4 address.
const ATYP_IPV4: u8 = 1;
/// The SOCKS5 address type of a domain name.
const ATYP_DOMAIN: u8 = 3;
/// The SOCKS5 address type of an IPv6 address.
const ATYP_IPV6: u8 = 4;

/// What one SOCKS5 client asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    address_type: u8,
    host: String,
    login: Option<(String, String)>,
}

/// A SOCKS5 server that notes the first request of each client and refuses it.
struct Socks5 {
    addr: SocketAddr,
    asked: Arc<Mutex<Vec<Asked>>>,
    task: JoinHandle<()>,
}

impl Drop for Socks5 {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Socks5 {
    async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds loopback");
        let addr = listener.local_addr().expect("an address");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&asked);
        let task = tokio::spawn(async move {
            while let Ok((tcp, _peer)) = listener.accept().await {
                let record = Arc::clone(&record);
                tokio::spawn(async move {
                    let _served = serve(tcp, &record).await;
                });
            }
        });
        Self { addr, asked, task }
    }

    fn asked(&self) -> Vec<Asked> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Reads exactly `count` bytes.
async fn take(tcp: &mut TcpStream, count: usize) -> Option<Vec<u8>> {
    let mut bytes = vec![0_u8; count];
    tcp.read_exact(&mut bytes).await.ok()?;
    Some(bytes)
}

/// Serves one client: the method choice, the login when it offers one, the request, and a
/// "connection refused" answer.
async fn serve(mut tcp: TcpStream, record: &Mutex<Vec<Asked>>) -> Option<()> {
    let greeting = take(&mut tcp, 2).await?;
    let methods = take(&mut tcp, usize::from(greeting[1])).await?;
    let login = if methods.contains(&2) {
        tcp.write_all(&[5, 2]).await.ok()?;
        let user_length = take(&mut tcp, 2).await?[1];
        let user = take(&mut tcp, usize::from(user_length)).await?;
        let password_length = take(&mut tcp, 1).await?[0];
        let password = take(&mut tcp, usize::from(password_length)).await?;
        tcp.write_all(&[1, 0]).await.ok()?;
        Some((
            String::from_utf8_lossy(&user).into_owned(),
            String::from_utf8_lossy(&password).into_owned(),
        ))
    } else {
        tcp.write_all(&[5, 0]).await.ok()?;
        None
    };
    let request = take(&mut tcp, 4).await?;
    let address_type = request[3];
    let host = match address_type {
        ATYP_IPV4 => {
            let octets: [u8; 4] = take(&mut tcp, 4).await?.try_into().ok()?;
            Ipv4Addr::from(octets).to_string()
        }
        ATYP_DOMAIN => {
            let length = take(&mut tcp, 1).await?[0];
            let name = take(&mut tcp, usize::from(length)).await?;
            String::from_utf8_lossy(&name).into_owned()
        }
        ATYP_IPV6 => {
            let octets: [u8; 16] = take(&mut tcp, 16).await?.try_into().ok()?;
            Ipv6Addr::from(octets).to_string()
        }
        _ => return None,
    };
    take(&mut tcp, 2).await?;
    record
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Asked {
            address_type,
            host,
            login,
        });
    tcp.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await.ok()
}

#[tokio::test]
async fn a_socks5_egress_is_asked_for_the_name_and_not_for_an_address_looked_up_here() {
    let socks = Socks5::start().await;
    let ca = new_ca();
    let egress = EgressRef::parse(&format!("socks5://alice:s3cret@{}", socks.addr), None)
        .expect("an egress");
    // The name has an address here. If the scraper looked it up, the proxy would be handed that
    // address, and the site's name would be known to this machine's resolver.
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .resolve(NAME, "127.0.0.1:9".parse().expect("an address"));
    let (mitm, _sink) = start_mitm(Some(egress), options).await;
    let mut tls = tunnel(&mitm, NAME, 443, &der(&ca)).await;

    let request = navigation(NAME, 443, &measured_user_agent(), &[]);
    let (head, _body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 502, "the egress refused the connection");
    let asked = socks.asked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(
        (asked[0].address_type, asked[0].host.as_str()),
        (ATYP_DOMAIN, NAME),
        "the proxy was given an address instead of the name"
    );
    assert_eq!(
        asked[0].login,
        Some(("alice".to_owned(), "s3cret".to_owned())),
        "the credentials go through the SOCKS handshake"
    );
}
