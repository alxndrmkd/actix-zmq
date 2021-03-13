use bytes::Bytes;
use smallvec::{smallvec, SmallVec};
use std::ops::{Deref, DerefMut, Shl, ShlAssign};

const DEFAULT_BUF_SIZE: usize = 5;

#[derive(Default, Debug, Clone)]
pub struct ZmqMessage(SmallVec<[Bytes; DEFAULT_BUF_SIZE]>);

impl ZmqMessage {
    pub fn new<B: Into<Bytes>>(part: B) -> Self {
        Self(smallvec![part.into()])
    }
}

impl Deref for ZmqMessage {
    type Target = SmallVec<[Bytes; DEFAULT_BUF_SIZE]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ZmqMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Into<Bytes>> From<T> for ZmqMessage {
    fn from(v: T) -> Self {
        ZmqMessage::new(v)
    }
}

impl<B: Into<Bytes>> Shl<B> for ZmqMessage {
    type Output = ZmqMessage;

    fn shl(mut self, rhs: B) -> Self::Output {
        self <<= rhs;
        self
    }
}

impl<B: Into<Bytes>> ShlAssign<B> for ZmqMessage {
    fn shl_assign(&mut self, rhs: B) {
        self.0.push(rhs.into())
    }
}
