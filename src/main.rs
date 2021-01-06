use color_eyre::eyre::Result;
use futures_util::future::try_join;
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use std::convert::Infallible;
use std::net::{SocketAddr, ToSocketAddrs};
use structopt::StructOpt;
use tokio_socks::IntoTargetAddr;

#[derive(StructOpt, Debug)]
#[structopt(name = "sthp")]
struct Cli {
    #[structopt(short, long, default_value = "8080")]
    /// port where Http proxy should listen
    port: u16,

    /// Socks5 proxy address
    #[structopt(short, long, default_value = "127.0.0.1:1080")]
    socks_address: String,
}

#[tokio::main]
async fn main() {
    let args = Cli::from_args();
    let socks_address = args.socks_address;
    let port = args.port;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let socks_address = socks_address.to_socket_addrs().unwrap().next().unwrap();
    let make_service = make_service_fn(move |_| {
        let socks_address = socks_address.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let socks_address = socks_address.clone();
                proxy(req, socks_address)
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_service);
    println!("Server is listening on {}", addr);
    if let Err(e) = server.await {
        eprintln!("{:?}", e);
    };
}
async fn proxy(req: Request<Body>, socks_address: SocketAddr) -> Result<Response<Body>> {
    let _response = Response::new(Body::empty());

    if req.method() == hyper::Method::CONNECT {
        tokio::task::spawn(async move {
            let plain = req.uri().authority().unwrap().as_str().to_string();
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    if let Err(e) = tunnel(upgraded, plain, socks_address).await {
                        eprintln!("server io error: {}", e);
                    };
                }
                Err(e) => eprintln!("upgrade error: {}", e),
            }
        });
        Ok(Response::new(Body::empty()))
    } else {
        Ok(Response::new(Body::empty()))
    }
}

async fn tunnel(
    upgraded: hyper::upgrade::Upgraded,
    target_addr: String,
    socks_address: SocketAddr,
) -> std::io::Result<()> {
    let socket_address = socks_address.to_socket_addrs().unwrap().next().unwrap();

    let target_addr = target_addr.into_target_addr();
    let target_addr = target_addr.unwrap();
    let socks_stream = tokio_socks::tcp::Socks5Stream::connect(socket_address, target_addr)
        .await
        .expect("Cannot Connect to Socks5 Server");

    let amounts = {
        let (mut server_rd, mut server_wr) = tokio::io::split(socks_stream);
        let (mut client_rd, mut client_wr) = tokio::io::split(upgraded);

        let client_to_server = tokio::io::copy(&mut client_rd, &mut server_wr);
        let server_to_client = tokio::io::copy(&mut server_rd, &mut client_wr);

        try_join(client_to_server, server_to_client).await
    };

    // Print message when done
    match amounts {
        Ok((from_client, from_server)) => {
            println!(
                "client wrote {} bytes and received {} bytes",
                from_client, from_server
            );
        }
        Err(e) => {
            eprintln!("tunnel error: {}", e);
        }
    };
    Ok(())
}