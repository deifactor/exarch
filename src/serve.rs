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

pub async fn serve(opt: ServeOpt) -> Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", opt.port))
        .await
        .context("failed to bind")?;

    let server = build_server(&opt).context("couldn't build server")?;
    let acceptor: TlsAcceptor = server.into();

    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream.context("bad stream")?;
        let acceptor = acceptor.clone();
        task::spawn(async {
            handle_stream(stream, acceptor)
                .await
                .map_err(|err| error!("Error while handling stream: {}", err))
        });
    }

    Ok(())
}

pub fn build_server(opt: &ServeOpt) -> Result<ServerConfig> {
    let certs = File::open(&opt.cert)
        .context("failed to open certificate")
        .and_then(|cert| {
            pemfile::certs(&mut BufReader::new(cert))
                .map_err(|_| anyhow!("certificate decoding error"))
        })?;
    let mut keys = File::open(&opt.key)
        .context("failed to open keyfile")
        .and_then(|key| {
            pemfile::pkcs8_private_keys(&mut BufReader::new(key))
                .map_err(|_| anyhow!("keyfile decoding error"))
        })?;
    let mut server_config = ServerConfig::new(NoClientAuth::new());
    server_config
        .set_single_cert(certs, keys.remove(0))
        .context("failed to use certificate")?;
    Ok(server_config)
}

async fn handle_stream(stream: TcpStream, acceptor: TlsAcceptor) -> Result<()> {
    let peer_addr = stream.peer_addr()?.ip();
    debug!("Got connection from {}", peer_addr);
    let mut tls_stream = acceptor
        .accept(stream)
        .await
        .context("failed tcp handshake")?;
    let url = read_request(&mut tls_stream).await?;
    info!("{} requested {}", peer_addr, url);
    tls_stream.write_all(&b"20 text/gemini\r\n"[..]).await?;
    tls_stream.write_all(&b"foo bar baz"[..]).await?;
    tls_stream.flush().await?;
    Ok(())
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
