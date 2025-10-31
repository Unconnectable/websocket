# 🚀 Version 0.1：聊天服务器设计文档

## 目标与工具安装

### 测试工具安装

```sh
sudo apt update
sudo apt install netcat-openbsd
sudo apt install telnet
```

---

## 启动与连接测试

### 启动服务器

```sh
cargo run # 启动服务器
```

### 客户端连接方法（多终端）

```sh
# 在另一个终端 terminal 1
telnet 127.0.0.1 8080
# terminal 2
telnet 127.0.0.1 8080
# terminal 3
telnet 127.0.0.1 8080

# 或者使用 nc

# terminal 1
nc 127.0.0.1 8080
# t2
nc 127.0.0.1 8080
# t3
nc 127.0.0.1 8080
```

### 服务器输出示例

```sh
# output
char server connect success!
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:56683,
    fd: 10,
} addr: 127.0.0.1:56683
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:56697,
    fd: 11,
} add
```

从监听一个端口开始，如果已经被监听则报错

### 🚫 地址已监听（Address Already in Use）输出

```sh
# output
TCPlistener listening 127.0.0.1:8080 Error! chat server error: Os {
    code: 98,
    kind: AddrInUse,
    message: "Address already in use",
}
```

kill 当前占用 `127.0.0.1:8080`的端口

```sh
usr@EVO-X1:~/websocket$ lsof -i :8080
COMMAND     PID     USER FD   TYPE DEVICE SIZE/OFF NODE NAME
nc        48149 usr  3u  IPv4 367207      0t0  TCP localhost:57249->localhost:http-alt (CLOSE_WAIT)
websocket 49953 usr  9u  IPv4 377049      0t0  TCP localhost:http-alt (LISTEN)
websocket 49953 usr 11u  IPv4 377495      0t0  TCP localhost:http-alt->localhost:56697 (ESTABLISHED)
nc        51252 usr  3u  IPv4 415775      0t0  TCP localhost:56697->localhost:http-alt (ESTABLISHED)
```

---

## 🧐 注意某些存在的问题

### 1. 异步操作中的锁持有问题

- **问题描述：** 整个都是异步操作，期间持有锁会导致问题。
- **解决思路：** 在一个**很短的生命周期**持有锁，给通讯录添加和修改。

```rust
.....
{
    //防止async中lock出问题 粗暴的使用mutex 这里 后续改进
    let mut contact_temp = contact.lock().unwrap();
    contact_temp.remove(&addr);
}
```

### 2. `mpsc::Sender` 比较问题

- **问题描述：** `mpsc::Sender` 不实现 `PartialEq` 这个 `trait`（即不能使用 `==` 直接比较）。
- **解决思路：** 在收集的时候就排除自己就行。

```rust
//收集多线程的当前的通讯录
let senders:Vec<mpsc::Sender<String>> = {
    let contact_temp = contact.lock().unwrap();
    //contact_temp.values().cloned().collect()

    contact_temp.iter()
    .filter(|(key, _)| **key != addr)  // 排除掉当前任务的地址对应的 Sender
    .map(|(_, sender)| sender.clone())
    .collect()
};
```

## 附录:`v0.1`代码

```rust
mod test;
mod test1;
use std::{ collections::HashMap, net::SocketAddr, sync::{ Arc, Mutex } };
use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, BufReader };
use tokio::net::{ TcpListener, TcpStream };
use tokio::sync::mpsc;

// --- 核心类型定义 ---
// 值（Value）现在是 mpsc::Sender 的克隆体，它实现了 Clone
type SharedContacts = Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>;
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
// const YELLOW: &str = "\x1b[33m";
// const BOLD: &str = "\x1b[1m";
#[tokio::main]
async fn main() -> () {
    let contact: SharedContacts = Arc::new(Mutex::new(HashMap::new()));
    let listener = match TcpListener::bind("127.0.0.1:8080").await {
        Ok(listener) => {
            println!("{GREEN}char server connect success!{RESET}");
            listener
        }
        Err(e) => {
            // 监听失败 println 错误msg 然后结束
            eprintln!(
                "{RED}TCPlistener listening 127.0.0.1:8080 Error! chat server error: {:#?}{RESET}",
                e
            );
            return;
        }
    };
    loop {
        //在循环中获取socker and addr  如果成功就返回
        //如果失败说明某个终端链接到服务器有问题 跳过即可
        let (socket, addr) = match listener.accept().await {
            Ok((socket, addr)) => {
                println!(
                    "{GREEN}>>> New client connected socket: {:#?} addr: {:#?}{RESET}",
                    socket,
                    addr
                );
                (socket, addr)
            }
            Err(e) => {
                eprintln!("{RED}Error connecting in server loop : {:#?}{RESET}", e);
                continue;
            }
        };
        //复制通讯录给每一个spawn的线程 和client 每一个连接加入的都会增加通讯录的addr
        //当需要发送消息 或 需要接受消息的时候需要使用通讯录
        // 如果有client 断开 那么这个多线程的通讯录需要删除他
        let contact_clone = contact.clone();

        tokio::spawn(async move {
            // match handle_connection(socket, addr, contact_clone) {
            //     Ok(_) => {
            //         //
            //         println!("{RED}Error connecting in server loop{RESET}");
            //     }
            //     Err(e) => {
            //         eprintln!(
            //             "Error: {:#?} handling connection from addr: {:#?} socket: {:#?}",
            //             e, addr, socket
            //         );
            //     }
            // }
            if let Err(e) = handle_connection(socket, addr, contact_clone).await {
                // 通常只会在客户端意外断开或代码逻辑错误时发生
                eprintln!("Error handling connection from {}: {}", addr, e);
            }
        });
    }

    //unimplemented!();
}

// 客户端连接处理函数
// 同样需要loop 在主动断开之前 每个clinet都需要
async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    contact: SharedContacts
) -> Result<(), Box<dyn std::error::Error>> {
    // tx:统一的发送的地方  rx 单独的接受的地方
    let (tx, mut rx) = mpsc::channel(100);

    //无法在异步状态持有lock
    //type SharedContacts = Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>;
    //我这里需要短暂的持有这个在多线程间的hashmap 然后向他添加数据 因为他需要处理来自tokio::spawn产生的东西
    //只有这一种办法吗
    {
        let mut contact_temp = contact.lock().unwrap();
        contact_temp.insert(addr, tx);
    }

    let (reader, mut writer) = socket.into_split();

    let mut reader = BufReader::new(reader);
    //需要读取的line
    let mut line = String::new();

    loop {
        tokio::select! {
                        //case1: 当前的clinet是发送方 需要读取当前的msg 然后分发给除了自己之外的所有clinet
                        send_msg = reader.read_line(&mut line) =>{
                            //
                            let bytes_read = match send_msg{
                                Ok(n) => n,
                                //如果读取当前的msg出现了错误 退出当前的clinet
                                Err(e) =>{
                                    eprintln!("Error : {:#?} sending msg{:#?}",e,addr);
                                    break;
                                }
                            };

                            //遇见ctrl+c 或者别的 当前的终端需要推出
                            if bytes_read == 0{
                                break;
                            }

                            //去除空格 转换为String·
                            let msg = line.trim().to_string();
                            println!("[IN] {}: {}", addr, msg);


                            //收集多线程的当前的通讯录
                            let senders:Vec<mpsc::Sender<String>> = {
                                let contact_temp = contact.lock().unwrap();
                                //contact_temp.values().cloned().collect()

                                contact_temp.iter()
                                .filter(|(key, _)| **key != addr)  // 排除掉当前任务的地址对应的 Sender
                                .map(|(_, sender)| sender.clone())
                                .collect()

                            };

                            //消费掉sender
                            for peer_tx in senders.into_iter(){
                                // mpsc::Sender 不实现 PartialEq（即不能使用 == 直接比较）
                                // if peer_tx != tx{
                                    if let Err(e) = peer_tx.send(msg.clone()).await{
                                        eprintln!("Failed to send to a peer: {}", e);
                                    }

                                //}
                            }

                            //发送完毕
                            line.clear();
                        }

                        //case2: 需要接别的client的消息 然后println到自己的屏
                        Some(_msg) = rx.recv()=>{
                            // writer.write_all(_msg.as_bytes()).await?;
                            // writer.write_all(b"\n").await?;
                            // writer.flush().await?;

                            //使用最粗暴的方式处理错误
                            if let Err(e) = writer.write_all(_msg.as_bytes()).await {
                                // 在日志中明确指出是哪个步骤失败了
                                eprintln!("[ERROR] [Step: WriteMsg] Error writing msg to {}: {}", addr, e);
                                return Err(e.into());
                            }

                            // 步骤 2: 写入换行符
                            if let Err(e) = writer.write_all(b"\n").await {
                                eprintln!("[ERROR] [Step: WriteNewline] Error writing newline to {}: {}", addr, e);
                                return Err(e.into());
                            }

                            // 步骤 3: 刷新
                            if let Err(e) = writer.flush().await {
                                eprintln!("[ERROR] [Step: Flush] Error flushing stream to {}: {}", addr, e);
                                return Err(e.into());
                            }
                        }
        }
    }
    //如果进入到这一步 说明已经结束了 需要从通讯录删除当前的id
    {
        //防止async中lock出问题 粗暴的使用mutex 这里 后续改进
        let mut contact_temp = contact.lock().unwrap();
        contact_temp.remove(&addr);
    }
    println!(
        "--- Client {} handler finished. Active connections: {}",
        addr,
        contact.lock().unwrap().len()
    );

    Ok(())
    //unimplemented!();
}
```
