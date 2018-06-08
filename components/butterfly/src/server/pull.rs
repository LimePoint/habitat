// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
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

//! The pull thread.
//!
//! This module handles pulling all the pushed rumors from every member off a ZMQ socket.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use prost::Message;
use zmq;

use error::Error;
use message::swim::{Rumor, Rumor_Type};
use protocol::swim::Rumor as ProtoRumor;
use rumor::{election::ElectionUpdate, RumorPayload, RumorType};
use server::Server;
use trace::TraceKind;
use ZMQ_CONTEXT;

/// Takes a reference to the server itself
pub struct Pull {
    pub server: Server,
}

impl Pull {
    /// Create a new Pull
    pub fn new(server: Server) -> Pull {
        Pull { server: server }
    }

    /// Run this thread. Creates a socket, binds to the `gossip_addr`, then processes messages as
    /// they are received. Uses a ZMQ pull socket, so inbound messages are fair-queued.
    pub fn run(&mut self) {
        let socket = (**ZMQ_CONTEXT)
            .as_mut()
            .socket(zmq::PULL)
            .expect("Failure to create the ZMQ pull socket");
        socket
            .set_linger(0)
            .expect("Failure to set the ZMQ Pull socket to not linger");
        socket
            .set_tcp_keepalive(0)
            .expect("Failure to set the ZMQ Pull socket to not use keepalive");
        socket
            .bind(&format!("tcp://{}", self.server.gossip_addr()))
            .expect("Failure to bind the ZMQ Pull socket to the port");
        'recv: loop {
            if self.server.pause.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            let msg = match socket.recv_msg(0) {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Error receiving message: {:?}", e);
                    continue 'recv;
                }
            };
            let payload = match self.server.unwrap_wire(&msg) {
                Ok(payload) => payload,
                Err(e) => {
                    // NOTE: In the future, we might want to blacklist people who send us
                    // garbage all the time.
                    error!("Error parsing protocol message: {:?}", e);
                    continue;
                }
            };
            let mut proto = match ProtoRumor::decode(&payload).map_err(Error::from) {
                Ok(proto) => proto,
                Err(e) => {
                    error!("Error parsing protocol message: {:?}", e);
                    continue 'recv;
                }
            };
            if self.server.check_blacklist(&proto.from_id) {
                warn!(
                    "Not processing message from {} - it is blacklisted",
                    proto.from_id
                );
                continue 'recv;
            }
            trace_it!(GOSSIP: &self.server, TraceKind::RecvRumor, &proto.from_id, &proto);
            match proto.payload {
                RumorPayload::Membership(membership) => {
                    self.server
                        .insert_member_from_rumor(membership.member, membership.health);
                }
                RumorPayload::Service(service) => {
                    self.server.insert_service(service);
                }
                RumorPayload::ServiceConfig(service_config) => {
                    self.server.insert_service_config(service_config);
                }
                RumorPayload::ServiceFile(service_file) => {
                    self.server.insert_service_file(service_file);
                }
                RumorPayload::Election(election) => match proto.type_ {
                    RumorType::Election => self.server.insert_election(election),
                    RumorType::ElectionUpdate => {
                        // Ideally the election update rumor is it's own thing and not a tagged
                        // derivation of election. It originally made sense to sort of "inherit"
                        // from `Election`, but once we upgraded to the Prost implementation of
                        // Protobuf we got the ability to pack a Rumor's payload as a Rust
                        // enumeration.  This essentially makes the `type` field moot, so now this
                        // looks a bit out of place.
                        self.server
                            .insert_update_election(ElectionUpdate::from(election));
                    }
                    _ => panic!("unknown election proto type"),
                },
                RumorPayload::Departure(departure) => {
                    self.server.insert_departure(departure);
                }
            }
        }
    }
}
