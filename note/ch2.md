## 异步通信调用链分析

#### 1. `contact_guard.iter()`

- **作用：** 创建迭代器
- **传入参数：** 无
- **输出数据类型：** `Iterator<Item = (&String, &ClientInfo)>`
- **目的：** 遍历 `HashMap` 中的每一个 K-V 对。

#### 2. `.filter(...)`

- **作用：** 核心逻辑：排除自己
- **传入参数：** `|(key, _)|`
- **输出数据类型：** 保持 `Iterator<Item = (&String, &ClientInfo)>`
- **目的：** 对每个元素进行过滤。`key` 是当前遍历到的用户名 (`&String`)。`_` 表示我们不关心 `ClientInfo` 的值。

#### 3. **过滤条件：** `key != username`

- **作用：** 过滤条件
- **传入参数：** `key` 是 `&String`，我们使用 `*key` 解引用得到 `String` 的引用 `&str`，然后再次解引用 `**key` 得到 `String` 的值，与当前的 `username` (也是 `String` 的值) 进行比较。
- **输出数据类型：** `bool`
- **目的：** 如果 `key` 与发送者 `username` 不相等，则保留该元素，实现 **自我排除**。

#### 4. `.map(...)`

- **作用：** 数据转换：提取 Sender
- **传入参数：** `|(_, info)|`
- **输出数据类型：** `Iterator<Item = mpsc::Sender<String>>`
- **目的：** 忽略 Key (`_`)，只关心 Value (`info`，即 `&ClientInfo`)。从 `info` 中提取 `tx` 字段，并使用 `.clone()` 复制 `mpsc::Sender`。

#### 5. `.collect()`

- **作用：** 收集结果
- **传入参数：** 无
- **输出数据类型：** `Vec<mpsc::Sender<String>>`
- **目的：** 将所有收集到的 `Sender` 放入一个 `Vec` 中，供后续迭代发送消息使用

---

# V0.2

## 一、用户身份认证与重试机制

V0.2 引入了严格的握手认证阶段，确保每个连接都有一个唯一的身份，并极大改善了用户输入错误时的体验。

### 1. 用户名唯一性保障与重试循环

现在，客户端在进入聊天室之前必须通过一个 **内嵌的 `loop`** 进行身份验证。如果用户输入了空用户名或重复的用户名，服务器会提示错误并要求用户重新输入，而不会立即断开连接，显著提升了用户体验。

**握手认证循环：**

```rust
let username: String = loop {
    username_line.clear();
    writer.write_all(b"Enter username: \n").await?;
    writer.flush().await?;

    if reader.read_line(&mut username_line).await? == 0 {
        return Ok(()); // 客户端断开连接，任务退出
    }

    let username = username_line.trim().to_string();
    if username.is_empty() {
        // 仅按回车，提示重试
        let err_msg = format!("{RED}用户名不能为空，请重新输入.{RESET}\n");
        writer.write_all(err_msg.as_bytes()).await?;
        continue;
    }

    // 检查用户名是否唯一（Mutex 临界区操作）
    let is_unique = {
        let mut contact_gard = contact.lock().unwrap();
        !contact_gard.contains_key(&username)
    };
    if is_unique {
        break username; // 验证成功，退出循环并返回用户名
    } else {
        // 用户名重复，提示重试
        let err_msg =
            format!("{RED}用户名 '{}' 已被占用，请更换后重新输入.{RESET}\n", username);
        writer.write_all(err_msg.as_bytes()).await?;
        continue; // 继续循环，要求重新输入
    }
};
```

### 2. `break expression` 语法应用

代码使用了 `break username;` 语法，这使得整个 `loop` 作为一个 **表达式** 返回一个值 (`username: String`)，实现了简洁地获取并确认最终通过验证的用户名。

## 二、核心数据结构与架构改进

### 1. 统一的客户端信息封装 (`ClientInfo`)

V0.2 新增了 `ClientInfo` 结构体，将一个客户端的所有关键信息（地址、身份、通信通道）整合到一起，提高了数据管理的统一性：

```rust
pub struct ClientInfo {
    pub addr: SocketAddr, // 客户端的网络地址
    pub username: String, // 客户端的用户名
    pub tx: mpsc::Sender<String>, // 客户端的专用收件箱 Sender
}
```

### 2. 全局通讯录 Key 升级

全局通讯录 `SharedContacts` 的 Key 从不具备业务意义的 `SocketAddr` 切换到了具备业务意义的 **`String` (用户名)**。

**架构定义：**

```rust
// 全局通讯录现在以 String (用户名) 为 Key
type SharedContacts = Arc<Mutex<HashMap<String, ClientInfo>>>;
```

## 三、I/O 健壮性改进

代码明确区分了两种输入终止状态，使得连接处理更加健壮：

1. **连接断开 (EOF) 检测：** 通过 `reader.read_line().await? == 0` 来检测客户端是否断开连接（例如按下了 `Ctrl+C`），此时应立即退出当前连接处理任务。
2. **空输入重试检测：** 通过 `username.is_empty()`（即 `trim()` 后仍为空），来判断用户是否仅按下了回车，此时应提示用户重试 (`continue`) 而非断开连接。

---

###

实际操作 在多个窗口`nc 127.0.0.1 8080`

```sh
char server connect success!
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:46742,
    fd: 10,
} addr: 127.0.0.1:46742
User 'tm1' (from 127.0.0.1:46742) 注册成功.
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:45788,
    fd: 11,
} addr: 127.0.0.1:45788
User 'tm2' (from 127.0.0.1:45788) 注册成功.
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:45802,
    fd: 12,
} addr: 127.0.0.1:45802
User 'tm3' (from 127.0.0.1:45802) 注册成功.
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:45816,
    fd: 13,
} addr: 127.0.0.1:45816
>>> New client connected socket: TcpStream {
    addr: 127.0.0.1:8080,
    peer: 127.0.0.1:47268,
    fd: 13,
} addr: 127.0.0.1:47268
User 'tm4' (from 127.0.0.1:47268) 注册成功.
[IN] tm4 广播: msg from tm4

```

别的终端

```sh
filament@EVO-X1:~/websocket$ nc 127.0.0.1 8080
Enter username:
tm2
用户名 'tm2' 已被占用，请更换后重新输入.
Enter username:
tm3
欢迎, tm3! 您现在可以聊天了.
```

## 附录:解耦前的代码

```rust
mod test;
mod test1;
mod test2;
use std::{ collections::HashMap, fs::read, net::SocketAddr, sync::{ Arc, Mutex } };
use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, BufReader };
use tokio::net::{ TcpListener, TcpStream };
use tokio::sync::mpsc;

// --- 核心类型定义 ---

// 定义一个结构体来保存每个客户端的所有信息
pub struct ClientInfo {
    pub addr: SocketAddr, // 客户端的网络地址
    pub username: String, // 客户端的用户名
    pub tx: mpsc::Sender<String>, // 客户端的专用收件箱 Sender
}

// 全局通讯录现在以 String (用户名) 为 Key
type SharedContacts = Arc<Mutex<HashMap<String, ClientInfo>>>;
//type SharedContacts = Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>;
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

    //无法在异步状态持有lock
    //type SharedContacts = Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<String>>>>;
    //我这里需要短暂的持有这个在多线程间的hashmap 然后向他添加数据 因为他需要处理来自tokio::spawn产生的东西
    //只有这一种办法吗

    let (tx, mut rx) = mpsc::channel(100);
    let (reader_stream, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader_stream);
    let mut username_line = String::new();

    let username: String = loop {
        username_line.clear();
        writer.write_all(b"Enter username: \n").await?;
        writer.flush().await?;

        if reader.read_line(&mut username_line).await? == 0 {
            return Ok(());
        }

        let username = username_line.trim().to_string();
        if username.is_empty() {
            let err_msg = format!("{RED}username is empty{RESET}\n");
            writer.write_all(err_msg.as_bytes()).await?;
            continue;
        }

        let is_unique = {
            let mut contact_gard = contact.lock().unwrap();
            !contact_gard.contains_key(&username)
        };
        if is_unique {
            break username;
        } else {
            let err_msg =
                format!("{RED}用户名 '{}' 已被占用，请更换后重新输入.{RESET}\n", username);
            writer.write_all(err_msg.as_bytes()).await?;
            continue; // 继续循环，要求重新输入
        }
    };

    //用户名不为空 检测是否重复
    {
        let mut contact_gard = contact.lock().unwrap();
        let client_info = ClientInfo {
            addr,
            username: username.clone(),
            tx,
        };
        contact_gard.insert(username.clone(), client_info);
    }

    //username不重复
    println!("{GREEN}User '{}' (from {}) 注册成功.{RESET}", username, addr);
    let welcome_msg = format!("{GREEN}欢迎, {}! 您现在可以聊天了.{RESET}\n", username);
    writer.write_all(welcome_msg.as_bytes()).await?;

    //需要读取的msg line
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
                            let raw_msg = line.trim().to_string();
                            let msg = format!("[{username}: {raw_msg}]");
                            println!("[IN] {} 广播: {}", username, raw_msg);

                            
                            //收集多线程的当前的通讯录
                            let senders:Vec<mpsc::Sender<String>> = {
                                let contact_guard = contact.lock().unwrap();
                                contact_guard.iter()
                                .filter(|(key,_)| **key != username)
                                .map(|(_,info)| info.tx.clone())
                                .collect()
                                //contact_temp.values().cloned().collect()

                                // contact_temp.iter()
                                // .filter(|(key, _)| **key != addr)  // 排除掉当前任务的地址对应的 Sender
                                // .map(|(_, sender)| sender.clone())
                                // .collect()
                                
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
        contact_temp.remove(&username);
    }
    println!(
        "--- User '{}' handler finished. 当前活跃连接数: {}{RESET}",
        addr,
        contact.lock().unwrap().len()
    );

    Ok(())
    //unimplemented!();
}
```
