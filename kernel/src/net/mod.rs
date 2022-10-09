use alloc::vec::Vec;

pub mod socket;


#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Sockaddr {
    sa_family: usize,
    sa_data: Vec<usize>,
}
