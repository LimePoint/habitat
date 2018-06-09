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

//! The inbound thread.
//!
//! This module handles all the inbound SWIM messages.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use prost::Message as ProstMessage;

use super::AckSender;
use protocol::swim::{Ack, Health, Ping, PingReq, Swim, SwimKind};
use server::{outbound, Server};
use trace::TraceKind;

/// Takes the Server and a channel to send received Acks to the outbound thread.
pub struct Inbound {
    pub server: Server,
    pub socket: UdpSocket,
    pub tx_outbound: AckSender,
}

impl Inbound {
    /// Create a new Inbound.
    pub fn new(server: Server, socket: UdpSocket, tx_outbound: AckSender) -> Inbound {
        Inbound {
            server: server,
            socket: socket,
            tx_outbound: tx_outbound,
        }
    }

    /// Run the thread. Listens for messages up to 1k in size, and then processes them accordingly.
    pub fn run(&self) {
        let mut recv_buffer: Vec<u8> = vec![0; 1024];
        loop {
            if self.server.pause.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            match self.socket.recv_from(&mut recv_buffer[..]) {
                Ok((length, addr)) => {
                    let swim_payload = match self.server.unwrap_wire(&recv_buffer[0..length]) {
                        Ok(swim_payload) => swim_payload,
                        Err(e) => {
                            // NOTE: In the future, we might want to blacklist people who send us
                            // garbage all the time.
                            error!("Error decoding protocol message, {}", e);
                            continue;
                        }
                    };
                    let msg = match Swim::decode(&swim_payload) {
                        Ok(msg) => msg,
                        Err(e) => {
                            // NOTE: In the future, we might want to blacklist people who send us
                            // garbage all the time.
                            error!("Error decoding protocol message, {}", e);
                            continue;
                        }
                    };
                    trace!("SWIM Message: {:?}", msg);
                    match msg.kind {
                        SwimKind::Ping(ping) => {
                            if self.server.check_blacklist(&ping.from.id) {
                                debug!(
                                    "Not processing message from {} - it is blacklisted",
                                    ping.from.id
                                );
                                continue;
                            }
                            self.process_ping(addr, ping);
                        }
                        SwimKind::Ack(ack) => {
                            if self.server.check_blacklist(&ack.from.id) && ack.forward_to.is_none()
                            {
                                debug!(
                                    "Not processing message from {} - it is blacklisted",
                                    ack.from.id
                                );
                                continue;
                            }
                            self.process_ack(addr, ack);
                        }
                        SwimKind::PingReq(pingreq) => {
                            if self.server.check_blacklist(&pingreq.from.id) {
                                debug!(
                                    "Not processing message from {} - it is blacklisted",
                                    pingreq.from.id
                                );
                                continue;
                            }
                            self.process_pingreq(addr, pingreq);
                        }
                    }
                }
                Err(e) => {
                    // TODO: We can't use magic numbers here because the Supervisor runs on more
                    // than one platform. I'm sure these were added as specific OS errors for Linux
                    // but we need to also handle Windows & Mac.
                    match e.raw_os_error() {
                        Some(35) | Some(11) | Some(10035) | Some(10060) => {
                            // This is the normal non-blocking result, or a timeout
                        }
                        Some(_) => {
                            error!("UDP Receive error: {}", e);
                            debug!("UDP Receive error debug: {:?}", e);
                        }
                        None => {
                            error!("UDP Receive error: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// Process pingreq messages.
    fn process_pingreq(&self, addr: SocketAddr, mut msg: PingReq) {
        trace_it!(SWIM: &self.server, TraceKind::RecvPingReq, &msg.from.id, addr, &msg);
        self.server
            .member_list
            .with_member(&msg.target.id, move |m| match m {
                Some(target) => {
                    // Set the route-back address to the one we received the pingreq from
                    msg.from.address = addr.ip().to_string();
                    outbound::ping(
                        &self.server,
                        &self.socket,
                        target,
                        target.swim_socket_address(),
                        Some(msg.from),
                    );
                }
                None => {
                    error!("PingReq request {:?} for invalid target", msg);
                    return;
                }
            });
    }

    /// Process ack messages; forwards to the outbound thread.
    fn process_ack(&self, addr: SocketAddr, mut msg: Ack) {
        trace_it!(SWIM: &self.server, TraceKind::RecvAck, &msg.from.id, addr, &msg);
        trace!("Ack from {}@{}", msg.from.id, addr);
        if let Some(ref forward_to) = msg.forward_to {
            if *self.server.member_id != forward_to.id {
                let forward_addr_str = format!("{}:{}", forward_to.address, forward_to.swim_port);
                let forward_to_addr = match forward_addr_str.parse() {
                    Ok(addr) => addr,
                    Err(e) => {
                        error!(
                            "Abandoning Ack forward: cannot parse member address: {}:{}, {}",
                            forward_to.address, forward_to.swim_port, e
                        );
                        return;
                    }
                };
                trace!(
                    "Forwarding Ack from {}@{} to {}@{}",
                    msg.from.id,
                    addr,
                    forward_to.id,
                    forward_to.address,
                );
                msg.from.address = addr.ip().to_string();
                outbound::forward_ack(&self.server, &self.socket, forward_to_addr, msg);
                return;
            }
        }
        match self.tx_outbound.send((addr, msg)) {
            Ok(()) => {}
            Err(e) => panic!("Outbound thread has died - this shouldn't happen: #{:?}", e),
        }
        for membership in msg.membership {
            self.server
                .insert_member_from_rumor(membership.member, membership.health);
        }
    }

    /// Process ping messages.
    fn process_ping(&self, addr: SocketAddr, mut msg: Ping) {
        trace_it!(SWIM: &self.server, TraceKind::RecvPing, &msg.from.id, addr, &msg);
        outbound::ack(&self.server, &self.socket, &msg.from, addr, msg.forward_to);
        // Populate the member for this sender with its remote address
        msg.from.address = addr.ip().to_string();
        trace!("Ping from {}@{}", msg.from.id, addr);
        if msg.from.departed {
            self.server.insert_member(msg.from, Health::Departed);
        } else {
            self.server.insert_member(msg.from, Health::Alive);
        }
        for membership in msg.membership {
            self.server
                .insert_member_from_rumor(membership.member, membership.health);
        }
    }
}
