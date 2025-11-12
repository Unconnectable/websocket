## 异步通信调用链分析

### 1. `contact_guard.iter()`

- **作用：** 创建迭代器
- **传入参数：** 无
- **输出数据类型：** `Iterator<Item = (&String, &ClientInfo)>`
- **目的：** 遍历 `HashMap` 中的每一个 K-V 对。

### 2. `.filter(...)`

- **作用：** 核心逻辑：排除自己
- **传入参数：** `|(key, _)|`
- **输出数据类型：** 保持 `Iterator<Item = (&String, &ClientInfo)>`
- **目的：** 对每个元素进行过滤。`key` 是当前遍历到的用户名 (`&String`)。`_` 表示我们不关心 `ClientInfo` 的值。

### 3. **过滤条件：** `key != username`

- **作用：** 过滤条件
- **传入参数：** `key` 是 `&String`，我们使用 `*key` 解引用得到 `String` 的引用 `&str`，然后再次解引用 `**key` 得到 `String` 的值，与当前的 `username` (也是 `String` 的值) 进行比较。
- **输出数据类型：** `bool`
- **目的：** 如果 `key` 与发送者 `username` 不相等，则保留该元素，实现 **自我排除**。

### 4. `.map(...)`

- **作用：** 数据转换：提取 Sender
- **传入参数：** `|(_, info)|`
- **输出数据类型：** `Iterator<Item = mpsc::Sender<String>>`
- **目的：** 忽略 Key (`_`)，只关心 Value (`info`，即 `&ClientInfo`)。从 `info` 中提取 `tx` 字段，并使用 `.clone()` 复制 `mpsc::Sender`。

### 5. `.collect()`

- **作用：** 收集结果
- **传入参数：** 无
- **输出数据类型：** `Vec<mpsc::Sender<String>>`
- **目的：** 将所有收集到的 `Sender` 放入一个 `Vec` 中，供后续迭代发送消息使用

---

###

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
tm1
欢迎, tm1! 您现在可以聊天了.
[tm4: msg from tm4]
msg from tm1
[tm2: msg from tm2]
```
