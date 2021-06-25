use actix_web::rt::System;
use actix_web::{get, App, HttpServer, Responder};
use anyhow::Result;
use log::*;
use std::net::ToSocketAddrs;
use std::process::ChildStdin;

pub fn start_web_server<Addr>(minecraft_server_stdin: ChildStdin, address: Addr)
where
	Addr: ToSocketAddrs + Send + 'static,
{
	std::thread::spawn(move || {
		System::new().block_on(async move {
			if let Err(e) = start_actix_server(minecraft_server_stdin, address).await {
				error!("Webserver exited with {:?}", e);
			}
		})
	});
}

async fn start_actix_server<Addr>(_minecraft_server_stdin: ChildStdin, address: Addr) -> Result<()>
where
	Addr: ToSocketAddrs + Send + 'static,
{
	HttpServer::new(|| App::new().service(index))
		.bind(address)?
		.run()
		.await?;

	Ok(())
}

#[get("/")]
async fn index() -> impl Responder {
	"Hello, World"
}
