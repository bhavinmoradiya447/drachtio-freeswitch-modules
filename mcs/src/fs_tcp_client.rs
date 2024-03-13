use std::future::Future;
use std::{io, iter};
use std::io::{Error, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use stubborn_io::ReconnectOptions;

use stubborn_io::tokio::{StubbornIo, UnderlyingIo};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::sleep;
use crate::CONFIG;
use tracing::{error, info};

#[derive(Debug)]
struct CommandClient(TcpStream);

impl<A> UnderlyingIo<A> for CommandClient
    where
        A: ToSocketAddrs + Sync + Send + Clone + Unpin + 'static,
{
    fn establish(ctor_arg: A) -> Pin<Box<dyn Future<Output=Result<Self, Error>> + Send>> {
        Box::pin(async move {
            let connect_result = TcpStream::connect(ctor_arg).await;

            match connect_result {
                Ok(mut stream) => {
                    let mut buf = [0; 128];
                    let auth_command = format!("auth {}\n\n", CONFIG.fs_esl_client.auth.clone());
                    let size = stream.read(&mut buf).await.unwrap();
                    let is_login_success = match String::from_utf8(buf[0..size].to_owned()) {
                        Ok(str) => {
                            match str.as_str().contains("Content-Type: auth/request") {
                                true => {
                                    stream.write(auth_command.as_bytes()).await.unwrap();
                                    stream.read(&mut buf).await.unwrap();
                                    match String::from_utf8(Vec::from(buf)) {
                                        Ok(str) => {
                                            match str.as_str().contains("Reply-Text: +OK accepted") {
                                                true => {
                                                    info!("Successfully login to ESL Socket");
                                                    true
                                                }
                                                _ => {
                                                    info!("Failed to login to ESL Socket {}", str);
                                                    stream.shutdown().await.unwrap();
                                                    false
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Got an error while login to ESL Socket {}", e);
                                            stream.shutdown().await.unwrap();
                                            false
                                        }
                                    }
                                }
                                false => {
                                    error!("Failed to get auth request on ESL Socket {}", str);
                                    stream.shutdown().await.unwrap();
                                    false
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to get auth request on ESL Socket {}", e);
                            false
                        }
                    };
                    if is_login_success {
                        Ok(CommandClient(stream))
                    } else {
                        Err(Error::new(io::ErrorKind::ConnectionRefused, "Connection Failed"))
                    }
                }
                Err(e) => {
                    error!("Failed to connect to ESL Socket {}", e);
                    Err(Error::new(io::ErrorKind::ConnectionRefused, "Connection Failed"))
                }
            }
        })
    }
}

impl AsyncRead for CommandClient {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        AsyncRead::poll_read(Pin::new(&mut self.0), cx, buf)
    }
}

impl AsyncWrite for CommandClient {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        AsyncWrite::poll_write(Pin::new(&mut self.0), cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.0), cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.0), cx)
    }

    fn poll_write_vectored(mut self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[IoSlice<'_>]) -> Poll<Result<usize, Error>> {
        AsyncWrite::poll_write_vectored(Pin::new(&mut self.0), cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        AsyncWrite::is_write_vectored(&self.0)
    }
}

async fn process_command(mut command_client: StubbornIo<CommandClient, String>, tx: UnboundedSender<String>, mut rx: UnboundedReceiver<String>) {
    while let Some(msg) = rx.recv().await {
        match command_client.write(format!("{}\n\n", msg).as_bytes()).await {
            Ok(size) => {
                info!("Sent command with size {}", size);
                let mut buf = [0; 100];
                let size = command_client.read(&mut buf).await.unwrap();
                info!("Got response {:?}", String::from_utf8(buf[0..size].to_owned()).unwrap())
            }
            Err(e) => {
                error!("Failed to send event to FS {}", e);
                sleep(Duration::from_secs(1)).await;
                tx.send(msg).unwrap();
            }
        }
    }
}


type FsCommandClient<A> = StubbornIo<CommandClient, A>;

pub async fn start_fs_esl_client(event_receiver: UnboundedReceiver<String>, event_sender: UnboundedSender<String>, host_name: String) -> Result<(), Box<dyn std::error::Error>> {
    let options = ReconnectOptions::new().with_exit_if_first_connect_fails(false).with_retries_generator(|| {
        iter::repeat(Duration::from_secs(1))
    });
    let stream: StubbornIo<CommandClient, String> = FsCommandClient::connect_with_options(host_name, options).await.unwrap();
    process_command(stream, event_sender, event_receiver).await;
    Ok(())
}