// Copyright (c) 2018 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use bytes::BytesMut;

use error::{Error, Result};
use member::{Member, Membership};
pub use protocol::swim::swim::Type as SwimType;
use protocol::{self,
               swim::{self, swim::Payload as SwimPayload}};

#[derive(Debug)]
pub struct Ack {
    pub from: Member,
    pub forward_to: Option<Member>,
}

impl protocol::Message for Ack {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let raw = swim::Swim::decode(bytes)?;
        let payload = match raw.payload.ok_or(Error::ProtocolMismatch("payload")?) {
            SwimPayload::Ack(payload) => payload,
            _ => panic!("from-bytes ack"),
        };
        Ok(Ack {
            from: raw.from.ok_or(Error::ProtocolMismatch("from"))?,
            forward_to: raw.forward_to,
        })
    }

    fn write_to_bytes(&self) -> Result<Vec<u8>> {
        let payload = swim::Ack {
            from: self.from.clone(),
            forward_to: self.forward_to.clone(),
        };
        let raw = swim::Swim {
            type_: SwimType::Ack as i32,
            membership: self.membership.clone(),
            payload: SwimPayload::Ack(payload),
        };
        let mut buf = BytesMut::with_capacity(raw.encoded_len());
        raw.encode(&mut buf)?;
        Ok(buf.to_vec())
    }
}

#[derive(Debug)]
pub struct Ping {
    pub from: Member,
    pub forward_to: Option<Member>,
}

#[derive(Debug)]
pub struct PingReq {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub target: Member,
}
