// use smoltcp::{socket::*, iface::SocketHandle};

// pub struct TcpSocketWrapper {
//     handle: SocketHandle,
//     is_listening: bool,
//     end_point: Option<IpEndpoint>
// }

// pub struct UdpSocketWrapper {
//     handle: Handle
// }

// pub struct UdpSocket {

// }


use alloc::{
    sync::Arc,
    vec::Vec,
};

use crate::{
    fs::{fifo::RingBuffer, file::SocketFile, File},
    proc::get_current_task,
    utils::Error,
};

type SockQueue = RingBuffer<Socket>;

// impl UdpSocketWrapper {
//     pub fn new() -> Self {
//         let rx_buffer = UdpSocketBuffer::new(
//             vec![UdpPacketMetadata::EMPTY; UDP_METADATA_BUF],
//             vec![0; UDP_RECVBUF],
//         );
//         let tx_buffer = UdpSocketBuffer::new(
//             vec![UdpPacketMetadata::EMPTY; UDP_METADATA_BUF],
//             vec![0; UDP_SENDBUF],
//         );
//         let socket = UdpSocket::new(rx_buffer, tx_buffer);
//     }
// }

enum Sockettype {
    TCP,
    UDP,
}

#[allow(unused)]
pub struct Socket {
    family: usize,
    socktype: Sockettype,
    protocol: usize,
    queue: SockQueue,
}

impl Drop for Socket {
    fn drop(&mut self) {}
}

impl Socket {
    /// family:         套接口协议族
    /// socktype:       套接口类型
    /// protocol:       传输层协议
    /// res:            输出参数，创建成功的套接口指针
    /// kern:           由内核还是应用程序创建
    pub fn sock_create(
        family: usize,
        socktype: usize,
        protocol: usize,
    ) -> Result<Arc<Self>, Error> {
        let t = match socktype {
            1 => Sockettype::TCP,
            2 => Sockettype::UDP,
            _ => Sockettype::TCP,
            // _ => return Err(Error::EPERM),
        };
        Ok(Arc::new(Socket {
            family,
            socktype: t,
            protocol,
            queue: SockQueue::new(),
        }))
    }
    pub fn sock_map_fd(self: Arc<Self>) -> Result<usize, Error> {
        let current = get_current_task().unwrap();
        /* 将file添加到fd_table中*/
        let fd_limit = current.get_max_fd();
        let fd = current.get_fd_table().add_file(self, fd_limit)?;
        Ok(fd as usize)
    }
    /// listen函数使socket处于被动的监听模式，并为该socket建立一个输入数据队列，将到达的服务请求保存在此队列中，直到程序处理它们
    pub fn listen(&self) -> Result<usize, Error> {
        Ok(0)
    }
    /// accept()函数让服务器接收客户的连接请求。在建立好输入队列后，服务器就调用accept函数，然后睡眠并等待客户的连接请求
    pub fn accept(&self) -> Result<usize, Error> {
        Ok(1)
    }
    /// 向连接的客户程序使用Connect函数来配置socket并与远端服务器建立一个TCP连接
    pub fn connect(&self) -> Result<usize, Error> {
        Ok(0)
    }
    pub fn getsockname(&self) -> Result<usize, Error> {
        Ok(0)
    }
    pub fn setsockopt(&self) -> Result<usize, Error> {
        Ok(0)
    }
    pub fn sendto(&self) -> Result<usize, Error> {
        Ok(1)
    }
    pub fn recvfrom(&self, _len: usize) -> Result<Vec<u8>, Error> {
        let mut a = Vec::new();
        a.push('x' as u8);
        Ok(a)
    }
}

impl File for Socket {
    fn close(&self) -> Result<(), Error> {
        panic!();
    }

    fn get_index(&self) -> Result<crate::fs::FileIndex, Error> {
        Err(Error::EINDEX)
    }

    fn read(&self, _len: usize) -> Result<Vec<u8>, Error> {
        Err(Error::EINVAL)
    }

    fn write(&self, _data: Vec<u8>) -> Result<usize, Error> {
        Err(Error::EINVAL)
    }

    fn readable(&self) -> bool {
        panic!();
    }

    fn writable(&self) -> bool {
        panic!();
    }

    fn seek(&self, _pos: usize, _mode: crate::fs::SeekMode) -> Result<isize, Error> {
        Err(Error::EPERM)
    }

    fn get_size(&self) -> Result<usize, Error> {
        panic!("no implement");
    }

    fn write_stat(&self, _stat: &crate::fs::FileStat) -> Result<(), Error> {
        Err(Error::EPERM)
    }

    fn read_stat(&self) -> Result<crate::fs::FileStat, Error> {
        Err(Error::EPERM)
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::vfs::VFS> {
        panic!();
    }

    fn as_dir<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile + 'a>, Error>
    where
        Self: 'a,
    {
        Err(Error::EPERM)
    }

    fn as_link<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> Result<alloc::sync::Arc<dyn crate::fs::file::LinkFile + 'a>, Error>
    where
        Self: 'a,
    {
        Err(Error::EPERM)
    }

    fn as_device<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> Result<alloc::sync::Arc<dyn crate::fs::DeviceFile + 'a>, Error>
    where
        Self: 'a,
    {
        Err(Error::EPERM)
    }

    fn as_block<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile + 'a>, Error>
    where
        Self: 'a,
    {
        Err(Error::EPERM)
    }

    fn as_char<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile + 'a>, Error>
    where
        Self: 'a,
    {
        Err(Error::EPERM)
    }

    fn as_fifo<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> Result<alloc::sync::Arc<dyn crate::fs::file::FIFOFile + 'a>, Error>
    where
        Self: 'a,
    {
        Err(Error::EPERM)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a>
    where
        Self: 'a,
    {
        self
    }

    fn as_any<'a>(
        self: alloc::sync::Arc<Self>,
    ) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync + 'a>
    where
        Self: 'a,
    {
        self
    }

    fn as_socket<'a>(self: Arc<Self>) -> Result<Arc<dyn SocketFile + 'a>, Error>
    where
        Self: 'a,
    {
        Ok(self)
    }
}

impl SocketFile for Socket {
    fn listen(&self) -> Result<usize, Error> {
        self.listen()
    }
    fn accept(&self) -> Result<usize, Error> {
        self.accept()
    }
    fn connect(&self) -> Result<usize, Error> {
        self.connect()
    }

    fn getsockname(&self) -> Result<usize, Error> {
        self.getsockname()
    }

    fn sendto(&self) -> Result<usize, Error> {
        self.sendto()
    }

    fn recvfrom(&self, len: usize) -> Result<Vec<u8>, Error> {
        self.recvfrom(len)
    }

    fn setsockopt(&self) -> Result<usize, Error> {
        self.setsockopt()
    }
}
