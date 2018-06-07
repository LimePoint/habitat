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

pub use self::gen::{membership::Health, swim::Payload as SwimPayload, swim::Type as SwimType,
                    Member, Membership, Swim};
use error::{Error, Result};
use member::{Member as CMember, Membership as CMembership};
use protocol;

impl From<CMember> for Member {
    fn from(value: CMember) -> Self {
        Member {
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

impl From<CMembership> for Membership {
    fn from(value: CMembership) -> Self {
        Membership {
            member: Some(value.member.into()),
            health: Some(value.health as i32),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Ack {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub forward_to: Option<Member>,
}

impl protocol::FromProto<Swim> for Ack {
    fn from_proto(value: Swim) -> Result<Self> {
        let payload = match value.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            SwimPayload::Ack(ack) => ack,
            _ => panic!("try-from ack"),
        };
        Ok(Ack {
            membership: value.membership.into_iter().map(Into::into).collect(),
            from: payload.from.ok_or(Error::ProtocolMismatch("from"))?.into(),
            forward_to: payload.forward_to.map(Into::into),
        })
    }
}

impl protocol::Message<Swim> for Ack {}

impl From<Ack> for Swim {
    fn from(value: Ack) -> Self {
        let payload = gen::Ack {
            from: Some(value.from.into()),
            forward_to: value.forward_to.into(),
        };
        Swim {
            type_: SwimType::Ack as i32,
            membership: value.membership.into(),
            payload: Some(SwimPayload::Ack(payload)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Ping {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub forward_to: Option<Member>,
}

impl protocol::FromProto<Swim> for Ping {
    fn from_proto(value: Swim) -> Result<Self> {
        let payload = match value.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            SwimPayload::Ping(ping) => ping,
            _ => panic!("try-from ping"),
        };
        Ok(Ping {
            membership: value.membership.into_iter().map(Into::into).collect(),
            from: payload.from.ok_or(Error::ProtocolMismatch("from"))?.into(),
            forward_to: payload.forward_to.map(Into::into),
        })
    }
}

impl protocol::Message<Swim> for Ping {}

impl From<Ping> for Swim {
    fn from(value: Ping) -> Self {
        let payload = gen::Ping {
            from: Some(value.from.into()),
            forward_to: value.forward_to.into(),
        };
        Swim {
            type_: SwimType::Ping as i32,
            membership: value.membership.into(),
            payload: Some(SwimPayload::Ping(payload)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PingReq {
    pub membership: Vec<Membership>,
    pub from: Member,
    pub target: Member,
}

impl protocol::FromProto<Swim> for PingReq {
    fn from_proto(value: Swim) -> Result<Self> {
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

impl protocol::Message<Swim> for PingReq {}

impl From<PingReq> for Swim {
    fn from(value: PingReq) -> Self {
        let payload = gen::PingReq {
            from: Some(value.from.into()),
            target: Some(value.target.into()),
        };
        Swim {
            type_: SwimType::Pingreq as i32,
            membership: value.membership.into(),
            payload: Some(SwimPayload::Pingreq(payload)),
        }
    }
}
