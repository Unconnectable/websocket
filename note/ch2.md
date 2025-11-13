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
