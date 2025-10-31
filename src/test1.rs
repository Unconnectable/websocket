use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

type State = Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>;
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
// const YELLOW: &str = "\x1b[33m";
// const BOLD: &str = "\x1b[1m";
#[tokio::main]
async fn main() {
    //
    let state: State = Arc::new(Mutex::new(HashMap::new()));

    let listener = match TcpListener::bind("127.0.0.1:8080").await {
        Ok(listenr) => {
            println!("{GREEN}chat server 0.1 connecting{RESET}");
            listenr
            //loop {}
        }
        Err(e) => {
            eprintln!(
                "{RED}TCPlistener listening Error! chat server error: {:#?}{RESET}",
                e
            );
            return;
        }
    };
    //启动服务器循环 持续接受多个终端连接
    loop {
        // 获取 socket 和 地址
        let (socket, addr) = match listener.accept().await {
            Ok((socket, addr)) => {
                println!("{GREEN}>>> New client connected: {}{RESET}", addr);
                (socket, addr)
            }
            Err(e) => {
                eprintln!("{RED}Error connecting : {:#?}{RESET}", e);
                continue;
            }
        };

        let state_clone = state.clone();

        tokio::spawn(async move {
            match handle_connection(socket, addr, state_clone).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error handling connection from {}: {:#?}", addr, e);
                }
            }
        });
    }
}
//async fn handle_connection(socket: TcpStream, addr: SocketAddr, state: State) -> Result<(), ()> {
async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    state: State,
) -> Result<(), Box<dyn std::error::Error>> {
    //Box<dyn std::error::Error>> {

    let (tx, mut rx) = mpsc::channel(100);

    //unimplemented!();

    {
        let mut map = state.lock().unwrap();
        map.insert(addr, tx);
    }

    let (reader, mut writer) = socket.into_split();

    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        tokio::select! {
            result = reader.read_line(&mut line) => {
                // let bytes_read = result.map_err(|e|{
                //     eprintln!("Error reading from {}: {:#?}",addr,e);
                // });
                let bytes_read = match result{
                    Ok(n) => n,
                    Err(e) =>{
                        eprintln!("Error reading from {}: {}", addr, e);
                        break;
                     }
                };

                if bytes_read == 0{
                    break;
                }
                //let msg = line.trim().into_string();
                let msg = line.trim().to_string(); // 使用 to_string() 转换为 String
                println!("[IN] {}: {}", addr, msg);


                let senders: Vec<mpsc::Sender<String>> = {
                  let map = state.lock().unwrap();
                  map.values().cloned().collect()
                };

                for peer_tx in senders.into_iter(){
                    if let Err(e) = peer_tx.send(msg.clone()).await{
                        eprintln!("Failed to send to a peer: {}", e);
                    }
                }
                line.clear();
            }

            // 接收广播的消息
            Some(msg)=rx.recv() =>{
                // writer.write_all(msg.as_bytes()).await.map_err(|addr| {
                //     eprintln!("Error writing to {}: {:#?}", addr, e);
                // });
                writer.write_all(msg.as_bytes()).await.map_err(|e| {
                    eprintln!("Error writing to {}: {:#?}", addr, e);
                });
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                //
            }
        }
    }

    {
        let mut map = state.lock().unwrap();
        map.remove(&addr);
    }
    println!(
        "--- Client {} handler finished. Active connections: {}",
        addr,
        state.lock().unwrap().len()
    );

    Ok(())
}
