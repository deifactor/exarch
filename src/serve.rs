use crate::markgem;
use anyhow::{anyhow, bail, Context, Result};
use async_std::io::prelude::*;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use async_tls::TlsAcceptor;
use log::{debug, error, info};
use rustls::{internal::pemfile, NoClientAuth, ServerConfig};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use url::Url;

#[derive(Debug, StructOpt)]
pub struct ServeOpt {
    /// Path to the TLS certificate.
    #[structopt(short, long, parse(from_os_str))]
    cert: PathBuf,

    /// Path to the TLS key file.
    #[structopt(short, long, parse(from_os_str))]
    key: PathBuf,

    /// The root of the tree to serve.
    #[structopt(parse(from_os_str))]
    root: PathBuf,

    /// What port to listen on.
    #[structopt(short, long, default_value = "1965")]
    port: u16,
}

pub async fn serve(options: ServeOpt) -> Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", options.port))
        .await
        .context("failed to bind")?;
    let server = Arc::new(Server::build(options).await?);

    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream.context("bad stream")?;
        server.clone().handle_stream(stream).await?;
    }

    Ok(())
}

struct Server {
    options: ServeOpt,
    acceptor: TlsAcceptor,
}

impl Server {
    async fn build(options: ServeOpt) -> Result<Self> {
        let certs = File::open(&options.cert)
            .context("failed to open certificate")
            .and_then(|cert| {
                pemfile::certs(&mut BufReader::new(cert))
                    .map_err(|_| anyhow!("certificate decoding error"))
            })?;
        let mut keys = File::open(&options.key)
            .context("failed to open keyfile")
            .and_then(|key| {
                pemfile::pkcs8_private_keys(&mut BufReader::new(key))
                    .map_err(|_| anyhow!("keyfile decoding error"))
            })?;
        let mut server_config = ServerConfig::new(NoClientAuth::new());
        server_config
            .set_single_cert(certs, keys.remove(0))
            .context("failed to use certificate")?;
        let acceptor: TlsAcceptor = server_config.into();
        Ok(Self { options, acceptor })
    }

    async fn handle_stream(self: Arc<Self>, stream: TcpStream) -> Result<()> {
        let acceptor = self.acceptor.clone();
        task::spawn(async {
            if let Err(e) = self.handle_inner(stream, acceptor).await {
                error!("Error while handling stream: {}", e);
            }
        });
        Ok(())
    }

    async fn handle_inner(self: Arc<Self>, stream: TcpStream, acceptor: TlsAcceptor) -> Result<()> {
        let peer_addr = stream.peer_addr()?.ip();
        debug!("Got connection from {}", peer_addr);
        let mut tls_stream = acceptor
            .accept(stream)
            .await
            .context("failed tcp handshake")?;
        let url = read_request(&mut tls_stream).await?;
        info!("{} requested {}", peer_addr, url);
        self.reply(url, &mut tls_stream).await?;
        tls_stream.flush().await?;
        Ok(())
    }

    async fn reply<W: Write + Unpin>(&self, url: Url, mut stream: W) -> Result<()> {
        let mut path = self.options.root.clone();
        if let Some(segments) = url.path_segments() {
            path.extend(segments);
        }
        debug!("Serving {}", path.display());
        let contents = std::fs::read_to_string(path)?;
        stream.write_all(&b"20 text/gemini\r\n"[..]).await?;
        let gemini = markgem::to_gemini(&contents)?;
        stream.write_all(&gemini).await?;
        Ok(())
    }
}

const MAX_URL_LENGTH: usize = 1024;
const EOL: &'static [u8] = b"\r\n";

async fn read_request<R: Read + Unpin>(mut stream: R) -> Result<Url> {
    // The longest valid request is a 1024-character URL followed by CRLF, so we can statically
    // allocate this many bytes.
    let mut request = [0; MAX_URL_LENGTH + EOL.len()];
    let mut len = 0;
    loop {
        let bytes_read = stream.read(&mut request[len..]).await?;
        len += bytes_read;
        if request[..len].ends_with(EOL) {
            // Got the full URL.
            break;
        } else if bytes_read == 0 {
            bail!("Unexpected end of request");
        }
    }
    let request = std::str::from_utf8(&request[..len - EOL.len()])
        .context("could not parse request as utf8")?;
    let mut url = Url::parse(request)?;
    if url.scheme() == "" {
        url.set_scheme("gemini")
            .map_err(|_| anyhow!("Could not set URL scheme"))?;
    }
    if url.scheme() != "gemini" {
        bail!("Unknown url scheme {}", url.scheme())
    }
    Ok(url)
}
