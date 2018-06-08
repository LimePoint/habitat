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

mod gen {
    include!("../generated/butterfly.swim.rs");
}

use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;

use bytes::BytesMut;
use prost::Message as ProstMessage;
use uuid::Uuid;

pub use self::gen::{membership::Health, swim::Payload as SwimPayload, swim::Type as SwimType};
use error::{Error, Result};
use protocol::{self, FromProto};
use rumor::{RumorEnvelope, RumorKey, RumorKind, RumorType};

#[derive(Debug, Clone, Serialize)]
pub struct Ack {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub forward_to: Option<Member>,
}

impl protocol::FromProto<gen::Swim> for Ack {
    fn from_proto(value: gen::Swim) -> Result<Self> {
        let payload = match value.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            SwimPayload::Ack(ack) => ack,
            _ => panic!("try-from ack"),
        };
        let forward_to = if let Some(forward_to) = payload.forward_to {
            Some(Member::from_proto(forward_to)?)
        } else {
            None
        };
        Ok(Ack {
            membership: value
                .membership
                .into_iter()
                .map(Membership::from_proto)
                .collect(),
            from: payload.from.ok_or(Error::ProtocolMismatch("from"))?.into(),
            forward_to: forward_to,
        })
    }
}

impl protocol::Message<gen::Swim> for Ack {}

impl From<Ack> for gen::Swim {
    fn from(value: Ack) -> Self {
        let payload = gen::Ack {
            from: Some(value.from.into()),
            forward_to: value.forward_to.into(),
        };
        gen::Swim {
            type_: SwimType::Ack as i32,
            membership: value.membership.into(),
            payload: Some(SwimPayload::Ack(payload)),
        }
    }
}

impl From<Ack> for Swim {
    fn from(value: Ack) -> Self {
        Swim {
            type_: SwimType::Ack,
            membership: value.membership.clone(),
            kind: SwimKind::Ack(value),
        }
    }
}

impl FromStr for Health {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_lowercase().as_ref() {
            "alive" => Ok(Health::Alive),
            "suspect" => Ok(Health::Suspect),
            "confirmed" => Ok(Health::Confirmed),
            "departed" => Ok(Health::Departed),
            value => panic!("No match for Health from string, {}", value),
        }
    }
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match *self {
            Health::Alive => "alive",
            Health::Suspect => "suspect",
            Health::Confirmed => "confirmed",
            Health::Departed => "departed",
        };
        write!(f, "{}", value)
    }
}

/// A member in the swim group. Passes most of its functionality along to the internal protobuf
/// representation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub incarnation: u64,
    pub address: String,
    pub swim_port: i32,
    pub gossip_port: i32,
    pub persistent: bool,
    pub departed: bool,
}

impl Member {
    /// Returns the socket address of this member.
    ///
    /// # Panics
    ///
    /// This function panics if the address is un-parseable. In practice, it shouldn't be
    /// un-parseable, since its set from the inbound socket directly.
    pub fn swim_socket_address(&self) -> SocketAddr {
        let address_str = format!("{}:{}", self.address, self.swim_port);
        match address_str.parse() {
            Ok(addr) => addr,
            Err(e) => {
                panic!("Cannot parse member {:?} address: {}", self, e);
            }
        }
    }
}

impl Default for Member {
    fn default() -> Self {
        Member {
            id: Uuid::new_v4().simple().to_string(),
            incarnation: 0,
            address: String::default(),
            swim_port: 0,
            gossip_port: 0,
            persistent: false,
            departed: false,
        }
    }
}

impl From<Member> for RumorKey {
    fn from(member: Member) -> RumorKey {
        RumorKey::new(RumorType::Member, member.id, "")
    }
}

impl<'a> From<&'a Member> for RumorKey {
    fn from(member: &'a Member) -> RumorKey {
        RumorKey::new(RumorType::Member, member.id, "")
    }
}

impl<'a> From<&'a &'a Member> for RumorKey {
    fn from(member: &'a &'a Member) -> RumorKey {
        RumorKey::new(RumorType::Member, member.id, "")
    }
}

impl From<Member> for gen::Member {
    fn from(value: Member) -> Self {
        gen::Member {
            id: Some(value.id),
            incarnation: Some(value.incarnation),
            address: Some(value.address),
            swim_port: Some(value.swim_port),
            gossip_port: Some(value.gossip_port),
            persistent: Some(value.persistent),
            departed: Some(value.departed),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    pub member: Member,
    pub health: Health,
}

impl Membership {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let rumor = RumorEnvelope::decode(bytes)?;
        match rumor.kind {
            RumorKind::Membership(payload) => Ok(payload),
            _ => panic!("from-bytes member"),
        }
        // let rumor = ProtoRumor::decode(bytes)?;
        // let payload = match rumor.payload.ok_or(Error::ProtocolMismatch("payload"))? {
        //     RumorPayload::Member(payload) => payload,
        //     _ => panic!("from-bytes member"),
        // };
        // let member = payload.member.ok_or(Error::ProtocolMismatch("member"))?;
        // Ok(Membership {
        //     member: Member {
        //         id: member.id.ok_or(Error::ProtocolMismatch("id"))?,
        //         incarnation: member.incarnation.unwrap_or(0),
        //         address: member.address.ok_or(Error::ProtocolMismatch("address"))?,
        //         swim_port: member
        //             .swim_port
        //             .ok_or(Error::ProtocolMismatch("swim-port"))?,
        //         gossip_port: member
        //             .gossip_port
        //             .ok_or(Error::ProtocolMismatch("gossip-port"))?,
        //         persistent: member.persistent.unwrap_or(false),
        //         departed: member.departed.unwrap_or(false),
        //     },
        //     health: payload
        //         .health
        //         .and_then(Health::from_i32)
        //         .unwrap_or(Health::Alive),
        // })
    }

    pub fn write_to_bytes(self) -> Result<Vec<u8>> {
        let rumor: gen::Membership = self.into();
        let mut bytes = BytesMut::with_capacity(rumor.encoded_len());
        Ok(bytes.to_vec())
    }
}

impl From<Membership> for gen::Membership {
    fn from(value: Membership) -> Self {
        gen::Membership {
            member: Some(value.member.into()),
            health: Some(value.health as i32),
        }
    }
}
impl FromProto<gen::Member> for Member {
    fn from_proto(proto: gen::Member) -> Result<Self> {
        Ok(Member {
            id: proto.id.ok_or(Error::ProtocolMismatch("id"))?,
            incarnation: proto.incarnation.unwrap_or(0),
            address: proto.address.ok_or(Error::ProtocolMismatch("address"))?,
            swim_port: proto.swim_port.ok_or(Error::ProtocolMismatch("swim-port"))?,
            gossip_port: proto
                .gossip_port
                .ok_or(Error::ProtocolMismatch("gossip-port"))?,
            persistent: proto.persistent.unwrap_or(false),
            departed: proto.departed.unwrap_or(false),
        })
    }
}

impl FromProto<gen::Membership> for Membership {
    fn from_proto(proto: gen::Membership) -> Result<Self> {
        Ok(Membership {
            member: proto
                .member
                .ok_or(Error::ProtocolMismatch("member"))
                .and_then(Member::from_proto)?,
            health: proto
                .health
                .and_then(Health::from_i32)
                .unwrap_or(Health::Alive),
        })
    }
}

// impl FromProto<ProtoRumor> for Membership {
//     fn from_proto(proto: ProtoRumor) -> Result<Self> {
//         match proto.payload {
//             RumorPayload::Member(membership) => Membership::from_proto(membership),
//             _ => panic!("from-proto payload"),
//         }
//     }
// }

#[derive(Debug, Clone, Serialize)]
pub struct Ping {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub forward_to: Option<Member>,
}

impl protocol::FromProto<gen::Swim> for Ping {
    fn from_proto(value: gen::Swim) -> Result<Self> {
        let payload = match value.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            SwimPayload::Ping(ping) => ping,
            _ => panic!("try-from ping"),
        };
        let forward_to = if let Some(forward_to) = payload.forward_to {
            Some(Member::from_proto(forward_to)?)
        } else {
            None
        };
        Ok(Ping {
            membership: value.membership.into_iter().map(Into::into).collect(),
            from: payload.from.ok_or(Error::ProtocolMismatch("from"))?.into(),
            forward_to: forward_to,
        })
    }
}

impl protocol::Message<gen::Swim> for Ping {}

impl From<Ping> for gen::Swim {
    fn from(value: Ping) -> Self {
        let payload = gen::Ping {
            from: Some(value.from.into()),
            forward_to: value.forward_to.map(Into::into),
        };
        gen::Swim {
            type_: SwimType::Ping as i32,
            membership: value.membership.into_iter().map(Into::into).collect(),
            payload: Some(SwimPayload::Ping(payload)),
        }
    }
}

impl From<Ping> for Swim {
    fn from(value: Ping) -> Self {
        Swim {
            type_: SwimType::Ping,
            membership: value.membership.clone(),
            kind: SwimKind::Ping(value),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PingReq {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub target: Member,
}

impl protocol::FromProto<gen::Swim> for PingReq {
    fn from_proto(value: gen::Swim) -> Result<Self> {
        let payload = match value.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            SwimPayload::Pingreq(ping) => ping,
            _ => panic!("try-from pingreq"),
        };
        Ok(PingReq {
            membership: value.membership.into_iter().map(Into::into).collect(),
            from: payload.from.ok_or(Error::ProtocolMismatch("from"))?.into(),
            target: payload
                .target
                .ok_or(Error::ProtocolMismatch("from"))?
                .into(),
        })
    }
}

impl protocol::Message<gen::Swim> for PingReq {}

impl From<PingReq> for gen::Swim {
    fn from(value: PingReq) -> Self {
        let payload = gen::PingReq {
            from: Some(value.from.into()),
            target: Some(value.target.into()),
        };
        gen::Swim {
            type_: SwimType::Pingreq as i32,
            membership: value.membership.into(),
            payload: Some(SwimPayload::Pingreq(payload)),
        }
    }
}

impl From<PingReq> for Swim {
    fn from(value: PingReq) -> Self {
        Swim {
            type_: SwimType::Pingreq,
            membership: value.membership.clone(),
            kind: SwimKind::PingReq(value),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum SwimKind {
    Ping(Ping),
    Ack(Ack),
    PingReq(PingReq),
}

#[derive(Debug, Clone, Serialize)]
pub struct Swim {
    pub type_: SwimType,
    pub membership: Vec<Membership>,
    pub kind: SwimKind,
}

impl Swim {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let proto = gen::Swim::decode(bytes)?;
        let type_ = SwimType::from_i32(proto.type_).ok_or(Error::ProtocolMismatch("type"))?;
        let kind = match type_ {
            SwimType::Ack => SwimKind::Ack(Ack::from_proto(proto)?),
            SwimType::Ping => SwimKind::Ping(Ping::from_proto(proto)?),
            SwimType::Pingreq => SwimKind::PingReq(PingReq::from_proto(proto)?),
        };
        Ok(Swim {
            type_: type_,
            membership: proto.membership.into_iter().map(Into::into).collect(),
            kind: kind,
        })
    }

    pub fn encode(self) -> Result<Vec<u8>> {
        let proto: gen::Swim = self.into();
        let mut buf = BytesMut::with_capacity(proto.encoded_len());
        proto.encode(&mut buf)?;
        Ok(buf.to_vec())
    }
}

impl From<Swim> for gen::Swim {
    fn from(value: Swim) -> Self {
        gen::Swim {
            type_: value.type_ as i32,
            membership: value.membership.into_iter().map(Into::into).collect(),
            payload: Some(value.kind.into()),
        }
    }
}
