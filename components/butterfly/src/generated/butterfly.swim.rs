#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Member {
    #[prost(string, optional, tag="1")]
    pub id: ::std::option::Option<String>,
    #[prost(uint64, optional, tag="2")]
    pub incarnation: ::std::option::Option<u64>,
    #[prost(string, optional, tag="3")]
    pub address: ::std::option::Option<String>,
    #[prost(int32, optional, tag="4")]
    pub swim_port: ::std::option::Option<i32>,
    #[prost(int32, optional, tag="5")]
    pub gossip_port: ::std::option::Option<i32>,
    #[prost(bool, optional, tag="6", default="false")]
    pub persistent: ::std::option::Option<bool>,
    #[prost(bool, optional, tag="7", default="false")]
    pub departed: ::std::option::Option<bool>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Ping {
    #[prost(message, optional, tag="1")]
    pub from: ::std::option::Option<Member>,
    #[prost(message, optional, tag="2")]
    pub forward_to: ::std::option::Option<Member>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Ack {
    #[prost(message, optional, tag="1")]
    pub from: ::std::option::Option<Member>,
    #[prost(message, optional, tag="2")]
    pub forward_to: ::std::option::Option<Member>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct PingReq {
    #[prost(message, optional, tag="1")]
    pub from: ::std::option::Option<Member>,
    #[prost(message, optional, tag="2")]
    pub target: ::std::option::Option<Member>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Membership {
    #[prost(message, optional, tag="1")]
    pub member: ::std::option::Option<Member>,
    #[prost(enumeration="membership::Health", optional, tag="2")]
    pub health: ::std::option::Option<i32>,
}
pub mod membership {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
    #[derive(Serialize, Deserialize, Hash)]
    pub enum Health {
        Alive = 1,
        Suspect = 2,
        Confirmed = 3,
        Departed = 4,
    }
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Election {
    #[prost(string, optional, tag="1")]
    pub member_id: ::std::option::Option<String>,
    #[prost(string, optional, tag="2")]
    pub service_group: ::std::option::Option<String>,
    #[prost(uint64, optional, tag="3")]
    pub term: ::std::option::Option<u64>,
    #[prost(uint64, optional, tag="4")]
    pub suitability: ::std::option::Option<u64>,
    #[prost(enumeration="election::Status", optional, tag="5")]
    pub status: ::std::option::Option<i32>,
    #[prost(string, repeated, tag="6")]
    pub votes: ::std::vec::Vec<String>,
}
pub mod election {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
    #[derive(Serialize, Deserialize, Hash)]
    pub enum Status {
        Running = 1,
        NoQuorum = 2,
        Finished = 3,
    }
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Service {
    #[prost(string, optional, tag="1")]
    pub member_id: ::std::option::Option<String>,
    #[prost(string, optional, tag="2")]
    pub service_group: ::std::option::Option<String>,
    #[prost(uint64, optional, tag="3")]
    pub incarnation: ::std::option::Option<u64>,
    #[prost(bool, optional, tag="8")]
    pub initialized: ::std::option::Option<bool>,
    #[prost(string, optional, tag="9")]
    pub pkg: ::std::option::Option<String>,
    #[prost(bytes, optional, tag="10")]
    pub cfg: ::std::option::Option<Vec<u8>>,
    #[prost(message, optional, tag="12")]
    pub sys: ::std::option::Option<SysInfo>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct ServiceConfig {
    #[prost(string, optional, tag="1")]
    pub service_group: ::std::option::Option<String>,
    #[prost(uint64, optional, tag="2")]
    pub incarnation: ::std::option::Option<u64>,
    #[prost(bool, optional, tag="3")]
    pub encrypted: ::std::option::Option<bool>,
    #[prost(bytes, optional, tag="4")]
    pub config: ::std::option::Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct ServiceFile {
    #[prost(string, optional, tag="1")]
    pub service_group: ::std::option::Option<String>,
    #[prost(uint64, optional, tag="2")]
    pub incarnation: ::std::option::Option<u64>,
    #[prost(bool, optional, tag="3")]
    pub encrypted: ::std::option::Option<bool>,
    #[prost(string, optional, tag="4")]
    pub filename: ::std::option::Option<String>,
    #[prost(bytes, optional, tag="5")]
    pub body: ::std::option::Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct SysInfo {
    #[prost(string, optional, tag="1", default="127.0.0.1")]
    pub ip: ::std::option::Option<String>,
    #[prost(string, optional, tag="2", default="localhost")]
    pub hostname: ::std::option::Option<String>,
    #[prost(string, optional, tag="3", default="127.0.0.1")]
    pub gossip_ip: ::std::option::Option<String>,
    #[prost(uint32, optional, tag="4")]
    pub gossip_port: ::std::option::Option<u32>,
    #[prost(string, optional, tag="5", default="127.0.0.1")]
    pub http_gateway_ip: ::std::option::Option<String>,
    #[prost(uint32, optional, tag="6")]
    pub http_gateway_port: ::std::option::Option<u32>,
    #[prost(string, optional, tag="7", default="127.0.0.1")]
    pub ctl_gateway_ip: ::std::option::Option<String>,
    #[prost(uint32, optional, tag="8", default="9632")]
    pub ctl_gateway_port: ::std::option::Option<u32>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Departure {
    #[prost(string, optional, tag="1")]
    pub member_id: ::std::option::Option<String>,
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Swim {
    /// Identifies which field is filled in.
    #[prost(enumeration="swim::Type", required, tag="1")]
    pub type_: i32,
    #[prost(message, repeated, tag="5")]
    pub membership: ::std::vec::Vec<Membership>,
    #[prost(oneof="swim::Payload", tags="2, 3, 4")]
    pub payload: ::std::option::Option<swim::Payload>,
}
pub mod swim {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
    #[derive(Serialize, Deserialize, Hash)]
    pub enum Type {
        Ping = 1,
        Ack = 2,
        Pingreq = 3,
    }
    #[derive(Clone, Oneof, PartialEq)]
    #[derive(Serialize, Deserialize, Hash)]
    pub enum Payload {
        #[prost(message, tag="2")]
        Ping(super::Ping),
        #[prost(message, tag="3")]
        Ack(super::Ack),
        #[prost(message, tag="4")]
        Pingreq(super::PingReq),
    }
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Rumor {
    #[prost(enumeration="rumor::Type", required, tag="1")]
    pub type_: i32,
    #[prost(string, repeated, tag="2")]
    pub tag: ::std::vec::Vec<String>,
    #[prost(string, optional, tag="3")]
    pub from_id: ::std::option::Option<String>,
    #[prost(oneof="rumor::Payload", tags="4, 5, 6, 7, 8, 9")]
    pub payload: ::std::option::Option<rumor::Payload>,
}
pub mod rumor {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
    #[derive(Serialize, Deserialize, Hash)]
    pub enum Type {
        Member = 1,
        Service = 2,
        Election = 3,
        ServiceConfig = 4,
        ServiceFile = 5,
        Fake = 6,
        Fake2 = 7,
        ElectionUpdate = 8,
        Departure = 9,
    }
    #[derive(Clone, Oneof, PartialEq)]
    #[derive(Serialize, Deserialize, Hash)]
    pub enum Payload {
        #[prost(message, tag="4")]
        Member(super::Membership),
        #[prost(message, tag="5")]
        Service(super::Service),
        #[prost(message, tag="6")]
        ServiceConfig(super::ServiceConfig),
        #[prost(message, tag="7")]
        ServiceFile(super::ServiceFile),
        #[prost(message, tag="8")]
        Election(super::Election),
        #[prost(message, tag="9")]
        Departure(super::Departure),
    }
}
#[derive(Clone, PartialEq, Message)]
#[derive(Serialize, Deserialize, Hash)]
pub struct Wire {
    #[prost(bool, optional, tag="1", default="false")]
    pub encrypted: ::std::option::Option<bool>,
    #[prost(bytes, optional, tag="2")]
    pub nonce: ::std::option::Option<Vec<u8>>,
    #[prost(bytes, optional, tag="3")]
    pub payload: ::std::option::Option<Vec<u8>>,
}
