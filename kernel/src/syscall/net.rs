use crate::memory::copyout_vec;
use crate::utils::Error;
use crate::net::socket::Socket;
use crate::proc::{get_current_task, get_current_user_token};

/// socket（）创建通信端点，
/// 并返回引用该端点的文件描述符。
/// 成功调用返回的文件描述符将是当前未为进程打开的编号最低的文件描述符。
pub fn sys_socket(family: usize, socktype: usize, protocol: usize) -> Result<isize, Error> {
    let sock = Socket::sock_create(family, socktype, protocol)?;
    let fd = sock.sock_map_fd().unwrap();
    Ok(fd as _)
}
/// Bind函数将socket与本机上的一个端口相关联，随后你就可以在该端口监听服务请求
pub fn sys_bind(_sockfd: usize, _addr: usize, _addrlen: usize) -> Result<isize, Error> {
    Ok(0)
}
/// listen函数使socket处于被动的监听模式，并为该socket建立一个输入数据队列，将到达的服务请求保存在此队列中，直到程序处理它们
pub fn sys_listen(sockfd: usize, _backlog: usize) -> Result<isize, Error> {
    // Ok(0)
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    socket.as_socket()?.listen().map(|a| a as _)
}

pub fn sys_accept(sockfd: usize, _addr: usize, _addrlen: usize) -> Result<isize, Error> {
    // Ok(1)
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    socket.as_socket()?.accept().map(|a| a as _)
}

pub fn sys_connect(sockfd: usize, _addr: usize, _addrlen: usize) -> Result<isize, Error> {
    // Ok(0)
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    socket.as_socket()?.connect().map(|a| a as _)
}

pub fn sys_getsockname(sockfd: usize, _addr: usize, _addrlen: usize) -> Result<isize, Error> {
    // Ok(0)
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    socket.as_socket()?.getsockname().map(|a| a as _)
}

pub fn sys_sendto(
    sockfd: usize,
    _buf: usize,
    _len: usize,
    _flags: usize,
    _addr: usize,
    _addrlen: usize,
) -> Result<isize, Error> {
    // Ok(1)
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    socket.as_socket()?.sendto().map(|a| a as _)
}

pub fn sys_recvfrom(
    sockfd: usize,
    buf: *mut u8,
    len: usize,
    _flags: usize,
    _addr: usize,
    _addrlen: usize,
) -> Result<isize, Error> {
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    let token = get_current_user_token();
    let data = socket.as_socket().unwrap().recvfrom(len)?;
    copyout_vec(token, buf, data)?;
    Ok(1)
    // socket.as_socket()?.recvfrom().map(|a| a as _)
}

pub fn sys_setsockopt(
    sockfd: usize,
    _level: usize,
    _optname: usize,
    _optval: usize,
    _optlen: usize,
) -> Result<isize, Error> {
    // Ok(0)
    let task = get_current_task().unwrap();
    let socket = task.get_fd_table().get_file(sockfd as u32)?;
    socket.as_socket()?.setsockopt().map(|a| a as _)
}
